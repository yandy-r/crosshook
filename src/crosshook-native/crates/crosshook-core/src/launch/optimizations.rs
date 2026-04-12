use std::collections::{BTreeMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::{
    catalog::{global_catalog, OptimizationCatalog},
    request::{LaunchRequest, ValidationError, METHOD_PROTON_RUN},
    runtime_helpers::{build_gamescope_args, DEFAULT_HOST_PATH},
};
use crate::platform;
use crate::profile::GamescopeConfig;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}

impl LaunchDirectives {
    pub fn is_empty(&self) -> bool {
        self.env.is_empty() && self.wrappers.is_empty()
    }
}

pub fn is_known_launch_optimization_id(option_id: &str) -> bool {
    global_catalog().is_known_id(option_id)
}

/// Resolves launch optimization directives for a given method using the global catalog.
///
/// Used by `resolve_launch_directives` for direct Proton launches and by
/// [`build_steam_launch_options_command`] so Steam "Launch Options" strings stay aligned
/// with the same env/wrapper semantics.
pub fn resolve_launch_directives_for_method(
    enabled_option_ids: &[String],
    resolved_method: &str,
) -> Result<LaunchDirectives, ValidationError> {
    if enabled_option_ids.is_empty() {
        return Ok(LaunchDirectives::default());
    }
    resolve_directives_with_catalog(enabled_option_ids, resolved_method, global_catalog())
}

/// Resolves launch optimization directives against a specific catalog.
///
/// Extracted for testability — tests pass a catalog built from `parse_catalog_toml`
/// instead of relying on the process-global `OnceLock`.
pub(crate) fn resolve_directives_with_catalog(
    enabled_option_ids: &[String],
    resolved_method: &str,
    catalog: &OptimizationCatalog,
) -> Result<LaunchDirectives, ValidationError> {
    if enabled_option_ids.is_empty() {
        return Ok(LaunchDirectives::default());
    }

    let mut seen_ids = HashSet::new();
    for option_id in enabled_option_ids {
        if !seen_ids.insert(option_id.as_str()) {
            return Err(ValidationError::DuplicateLaunchOptimization(
                option_id.clone(),
            ));
        }

        if !catalog.is_known_id(option_id) {
            return Err(ValidationError::UnknownLaunchOptimization(
                option_id.clone(),
            ));
        }
    }

    let selected_ids = seen_ids;
    let mut directives = LaunchDirectives::default();

    for entry in &catalog.entries {
        if !selected_ids.contains(entry.id.as_str()) {
            continue;
        }

        if entry.applies_to_method != resolved_method {
            return Err(ValidationError::LaunchOptimizationNotSupportedForMethod {
                option_id: entry.id.clone(),
                method: resolved_method.to_string(),
            });
        }

        for conflicting_id in &entry.conflicts_with {
            if selected_ids.contains(conflicting_id.as_str()) {
                return Err(ValidationError::IncompatibleLaunchOptimizations {
                    first: entry.id.clone(),
                    second: conflicting_id.clone(),
                });
            }
        }

        if !entry.required_binary.is_empty() && !is_command_available(&entry.required_binary) {
            return Err(ValidationError::LaunchOptimizationDependencyMissing {
                option_id: entry.id.clone(),
                dependency: entry.required_binary.clone(),
            });
        }

        for pair in &entry.env {
            let key = &pair[0];
            let value = &pair[1];

            if !catalog.allowed_env_keys.contains(key.as_str()) {
                return Err(ValidationError::UnknownLaunchOptimization(entry.id.clone()));
            }

            directives.env.push((key.clone(), value.clone()));
        }

        for wrapper in &entry.wrappers {
            directives.wrappers.push(wrapper.clone());
        }
    }

    Ok(directives)
}

pub fn resolve_launch_directives(
    request: &LaunchRequest,
) -> Result<LaunchDirectives, ValidationError> {
    let enabled_option_ids = &request.optimizations.enabled_option_ids;
    if enabled_option_ids.is_empty() {
        return Ok(LaunchDirectives::default());
    }

    let resolved_method = request.resolved_method();
    if resolved_method != METHOD_PROTON_RUN {
        return Err(ValidationError::LaunchOptimizationsUnsupportedForMethod(
            resolved_method.to_string(),
        ));
    }

    resolve_launch_directives_for_method(enabled_option_ids, resolved_method)
}

/// Escapes a Steam launch-options env **value** (the portion after `KEY=`) so a space-separated
/// prefix line stays a single token per assignment when Steam/shell parses it.
///
/// Safe bare values are emitted unchanged. Values containing whitespace or shell-sensitive
/// characters are wrapped in double quotes with minimal backslash escapes inside the quotes.
pub fn escape_steam_token(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.chars().any(|ch| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '$' | ';' | '"' | '\'' | '\\' | '`' | '\n' | '\r' | '|' | '&' | '<' | '>'
                )
        });

    if !needs_quotes {
        return value.to_string();
    }

    let mut out = String::with_capacity(value.len().saturating_add(2));
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => out.push_str("\\$"),
            '`' => out.push_str("\\`"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Builds a single-line Steam per-game "Launch Options" string: `KEY=val ... wrappers %command%`.
///
/// Uses the same option-ID → env/wrapper mapping as `proton_run` launches, then appends
/// `custom_env_vars` as `KEY=value` tokens so profile custom values override optimization keys
/// when Steam evaluates the prefix (later assignments win).
///
/// When `gamescope` is `Some` and enabled, the gamescope compositor is inserted as a wrapper
/// before `%command%`: `gamescope [args] -- [other wrappers] %command%`. If `mangohud` is among
/// the optimization wrappers, it is replaced with `--mangoapp` on the gamescope args (same swap
/// as the `proton_run` launch path).
pub fn build_steam_launch_options_command(
    enabled_option_ids: &[String],
    custom_env_vars: &BTreeMap<String, String>,
    gamescope: Option<&GamescopeConfig>,
) -> Result<String, ValidationError> {
    let directives = resolve_launch_directives_for_method(enabled_option_ids, METHOD_PROTON_RUN)?;
    let mut parts: Vec<String> = directives
        .env
        .iter()
        .map(|(key, value)| format!("{}={}", key, escape_steam_token(value)))
        .collect();
    for (key, value) in custom_env_vars {
        let trimmed_key = key.trim();
        if trimmed_key.is_empty() {
            continue;
        }
        parts.push(format!(
            "{}={}",
            trimmed_key,
            escape_steam_token(value.as_str())
        ));
    }

    let gamescope_active = gamescope.is_some_and(|cfg| cfg.enabled);
    if gamescope_active {
        let cfg = gamescope.unwrap();
        let mut gs_args = build_gamescope_args(cfg);

        // Mangohud↔mangoapp swap: when gamescope wraps the process, mangohud is replaced with
        // gamescope's built-in --mangoapp integration (same logic as proton_run in script_runner).
        let has_mangohud = directives.wrappers.iter().any(|w| w.trim() == "mangohud");
        if has_mangohud {
            gs_args.push("--mangoapp".into());
        }

        parts.push("gamescope".to_string());
        for arg in &gs_args {
            parts.push(escape_steam_token(arg));
        }
        parts.push("--".to_string());

        // Add remaining wrappers, excluding mangohud if swapped to mangoapp.
        let wrappers: Vec<&String> = if has_mangohud {
            directives
                .wrappers
                .iter()
                .filter(|w| w.trim() != "mangohud")
                .collect()
        } else {
            directives.wrappers.iter().collect()
        };
        parts.extend(wrappers.into_iter().cloned());
    } else {
        parts.extend(directives.wrappers);
    }

    parts.push("%command%".to_string());
    Ok(parts.join(" "))
}

pub(crate) fn is_command_available(binary: &str) -> bool {
    #[cfg(test)]
    {
        let guard = test_command_search_path()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(search_path) = guard.as_ref() {
            return is_executable_file(&search_path.join(binary));
        }
    }

    if platform::is_flatpak() {
        return platform::host_command_exists(binary);
    }

    let path_value = env::var_os("PATH").unwrap_or_else(|| OsString::from(DEFAULT_HOST_PATH));

    env::split_paths(&path_value).any(|directory| is_executable_file(&directory.join(binary)))
}

#[cfg(test)]
fn test_command_search_path() -> &'static Mutex<Option<PathBuf>> {
    static SEARCH_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    SEARCH_PATH.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn swap_test_command_search_path(next: Option<PathBuf>) -> Option<PathBuf> {
    let mut guard = test_command_search_path()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::mem::replace(&mut *guard, next)
}

fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::catalog;

    fn make_test_catalog() -> OptimizationCatalog {
        let (entries, warnings) =
            catalog::parse_catalog_toml(catalog::DEFAULT_CATALOG_TOML, "test");
        assert!(
            warnings.is_empty(),
            "default catalog must parse cleanly: {warnings:?}"
        );
        OptimizationCatalog::from_entries(entries)
    }

    fn write_executable_file(path: &Path) {
        fs::write(path, b"test").expect("write file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    #[test]
    fn resolves_env_directives_in_catalog_order() {
        let catalog = make_test_catalog();
        let ids = vec!["enable_hdr".to_string(), "disable_steam_input".to_string()];

        let directives = resolve_directives_with_catalog(&ids, METHOD_PROTON_RUN, &catalog)
            .expect("resolve directives");

        assert_eq!(
            directives.env,
            vec![
                ("PROTON_NO_STEAMINPUT".to_string(), "1".to_string()),
                ("PROTON_ENABLE_HDR".to_string(), "1".to_string()),
            ]
        );
        assert!(directives.wrappers.is_empty());
    }

    #[test]
    fn resolves_wrapper_directives_in_deterministic_order() {
        let catalog = make_test_catalog();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mangohud_path = temp_dir.path().join("mangohud");
        let gamemode_path = temp_dir.path().join("gamemoderun");
        write_executable_file(&mangohud_path);
        write_executable_file(&gamemode_path);
        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let ids = vec![
            "use_gamemode".to_string(),
            "show_mangohud_overlay".to_string(),
        ];
        let directives = resolve_directives_with_catalog(&ids, METHOD_PROTON_RUN, &catalog)
            .expect("resolve directives");

        assert_eq!(directives.wrappers, vec!["mangohud", "gamemoderun"]);
        assert!(directives.env.is_empty());
    }

    #[test]
    fn resolves_issue_58_env_directives_in_catalog_order() {
        let catalog = make_test_catalog();
        let ids: Vec<String> = vec![
            "enable_vkd3d_dxr",
            "disable_esync",
            "enable_dxvk_async",
            "enable_nvapi",
            "cap_dxvk_frame_rate",
            "force_large_address_aware",
            "enable_proton_log",
            "disable_fsync",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();

        let directives = resolve_directives_with_catalog(&ids, METHOD_PROTON_RUN, &catalog)
            .expect("resolve directives");

        assert_eq!(
            directives.env,
            vec![
                ("PROTON_NO_ESYNC".to_string(), "1".to_string()),
                ("PROTON_NO_FSYNC".to_string(), "1".to_string()),
                ("PROTON_ENABLE_NVAPI".to_string(), "1".to_string()),
                (
                    "PROTON_FORCE_LARGE_ADDRESS_AWARE".to_string(),
                    "1".to_string()
                ),
                ("PROTON_LOG".to_string(), "1".to_string()),
                ("DXVK_ASYNC".to_string(), "1".to_string()),
                ("DXVK_FRAME_RATE".to_string(), "60".to_string()),
                ("VKD3D_CONFIG".to_string(), "dxr".to_string()),
            ]
        );
        assert!(directives.wrappers.is_empty());
    }

    #[test]
    fn rejects_missing_wrapper_dependency() {
        let catalog = make_test_catalog();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let ids = vec!["show_mangohud_overlay".to_string()];
        let error = resolve_directives_with_catalog(&ids, METHOD_PROTON_RUN, &catalog)
            .expect_err("missing wrapper should fail");

        assert_eq!(
            error,
            ValidationError::LaunchOptimizationDependencyMissing {
                option_id: "show_mangohud_overlay".to_string(),
                dependency: "mangohud".to_string(),
            }
        );
    }

    #[test]
    fn steam_launch_options_empty_is_percent_command_percent() {
        // This test uses global_catalog via resolve_launch_directives_for_method,
        // but with empty IDs it short-circuits before touching the catalog.
        let command = build_steam_launch_options_command(&[], &BTreeMap::new(), None)
            .expect("empty steam command");
        assert_eq!(command, "%command%");
    }

    #[test]
    fn escape_steam_token_quotes_values_with_whitespace_and_metacharacters() {
        assert_eq!(escape_steam_token("1"), "1");
        assert_eq!(escape_steam_token("a b"), "\"a b\"");
        assert_eq!(escape_steam_token("a$b"), "\"a\\$b\"");
        assert_eq!(escape_steam_token("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(escape_steam_token("x\ny"), "\"x\\ny\"");
        assert_eq!(escape_steam_token(""), "\"\"");
    }

    #[test]
    fn steam_launch_options_trims_custom_env_keys() {
        // Empty optimization IDs — short-circuits before catalog access
        let custom = BTreeMap::from([(" DXVK_ASYNC ".to_string(), "1".to_string())]);
        let command =
            build_steam_launch_options_command(&[], &custom, None).expect("steam command");
        assert_eq!(command, "DXVK_ASYNC=1 %command%");
    }

    #[test]
    fn steam_launch_options_escapes_custom_values_with_shell_sensitive_chars() {
        // Empty optimization IDs — short-circuits before catalog access
        let custom = BTreeMap::from([("FOO".to_string(), "a b".to_string())]);
        let command =
            build_steam_launch_options_command(&[], &custom, None).expect("steam command");
        assert_eq!(command, "FOO=\"a b\" %command%");
    }

    #[test]
    fn steam_launch_options_with_gamescope_basic() {
        let cfg = GamescopeConfig {
            enabled: true,
            internal_width: Some(2560),
            internal_height: Some(1440),
            fullscreen: true,
            ..Default::default()
        };
        let command = build_steam_launch_options_command(&[], &BTreeMap::new(), Some(&cfg))
            .expect("steam command with gamescope");
        assert_eq!(command, "gamescope -w 2560 -h 1440 -f -- %command%");
    }

    #[test]
    fn steam_launch_options_gamescope_disabled_unchanged() {
        let cfg = GamescopeConfig {
            enabled: false,
            internal_width: Some(1920),
            internal_height: Some(1080),
            ..Default::default()
        };
        let command = build_steam_launch_options_command(&[], &BTreeMap::new(), Some(&cfg))
            .expect("steam command with disabled gamescope");
        assert_eq!(command, "%command%");
    }

    #[test]
    fn steam_launch_options_gamescope_mangohud_swap() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mangohud_path = temp_dir.path().join("mangohud");
        write_executable_file(&mangohud_path);
        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let cfg = GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        };
        let ids = vec!["show_mangohud_overlay".to_string()];
        let command = build_steam_launch_options_command(&ids, &BTreeMap::new(), Some(&cfg))
            .expect("steam command with gamescope + mangohud");
        // mangohud should be removed from wrappers and --mangoapp added to gamescope args
        assert_eq!(command, "gamescope -f --mangoapp -- %command%");
        assert!(!command.contains(" mangohud "));
    }

    #[test]
    fn steam_launch_options_gamescope_extra_args_escaped() {
        let cfg = GamescopeConfig {
            enabled: true,
            extra_args: vec!["--some flag".to_string()],
            ..Default::default()
        };
        let command = build_steam_launch_options_command(&[], &BTreeMap::new(), Some(&cfg))
            .expect("steam command with extra args");
        assert_eq!(command, "gamescope \"--some flag\" -- %command%");
    }
}
