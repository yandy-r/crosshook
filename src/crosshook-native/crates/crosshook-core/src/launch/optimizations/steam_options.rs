use std::collections::BTreeMap;

use crate::profile::GamescopeConfig;

use super::super::{
    request::{ValidationError, METHOD_PROTON_RUN},
    runtime_helpers::build_gamescope_args,
};
use super::directives::resolve_launch_directives_for_method;

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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_executable_file(path: &std::path::Path) {
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
