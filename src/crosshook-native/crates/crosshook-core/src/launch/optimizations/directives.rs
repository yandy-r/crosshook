use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::super::{
    catalog::{global_catalog, OptimizationCatalog},
    request::{LaunchRequest, ValidationError, METHOD_PROTON_RUN},
};
use super::command_check::is_command_available;

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

#[cfg(test)]
mod tests {
    use std::fs;

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
}
