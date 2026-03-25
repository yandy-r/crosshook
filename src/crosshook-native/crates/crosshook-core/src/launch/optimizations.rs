use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use super::{
    request::{LaunchRequest, ValidationError, METHOD_PROTON_RUN},
    env::LAUNCH_OPTIMIZATION_ENV_VARS,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}

impl LaunchDirectives {
    pub fn is_empty(&self) -> bool {
        self.env.is_empty() && self.wrappers.is_empty()
    }
}

struct LaunchOptimizationDefinition {
    id: &'static str,
    applies_to_method: &'static str,
    env: &'static [(&'static str, &'static str)],
    wrappers: &'static [&'static str],
    conflicts_with: &'static [&'static str],
    required_binary: Option<&'static str>,
}

const LAUNCH_OPTIMIZATION_DEFINITIONS: &[LaunchOptimizationDefinition] = &[
    LaunchOptimizationDefinition {
        id: "disable_steam_input",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_NO_STEAMINPUT", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "prefer_sdl_input",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_PREFER_SDL", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "hide_window_decorations",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_NO_WM_DECORATION", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "show_mangohud_overlay",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[],
        wrappers: &["mangohud"],
        conflicts_with: &[],
        required_binary: Some("mangohud"),
    },
    LaunchOptimizationDefinition {
        id: "use_gamemode",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[],
        wrappers: &["gamemoderun"],
        conflicts_with: &["use_game_performance"],
        required_binary: Some("gamemoderun"),
    },
    LaunchOptimizationDefinition {
        id: "use_game_performance",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[],
        wrappers: &["game-performance"],
        conflicts_with: &["use_gamemode"],
        required_binary: Some("game-performance"),
    },
    LaunchOptimizationDefinition {
        id: "enable_hdr",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_ENABLE_HDR", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_wayland_driver",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_ENABLE_WAYLAND", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "use_ntsync",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_USE_NTSYNC", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_local_shader_cache",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_LOCAL_SHADER_CACHE", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_fsr4_upgrade",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_FSR4_UPGRADE", "1")],
        wrappers: &[],
        conflicts_with: &["enable_fsr4_rdna3_upgrade"],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_fsr4_rdna3_upgrade",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_FSR4_RDNA3_UPGRADE", "1")],
        wrappers: &[],
        conflicts_with: &["enable_fsr4_upgrade"],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_xess_upgrade",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_XESS_UPGRADE", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_dlss_upgrade",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_DLSS_UPGRADE", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "show_dlss_indicator",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_DLSS_INDICATOR", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "enable_nvidia_libs",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("PROTON_NVIDIA_LIBS", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
    LaunchOptimizationDefinition {
        id: "steamdeck_compat_mode",
        applies_to_method: METHOD_PROTON_RUN,
        env: &[("SteamDeck", "1")],
        wrappers: &[],
        conflicts_with: &[],
        required_binary: None,
    },
];

pub fn resolve_launch_directives(request: &LaunchRequest) -> Result<LaunchDirectives, ValidationError> {
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

    let mut seen_ids = HashSet::new();
    for option_id in enabled_option_ids {
        if !seen_ids.insert(option_id.as_str()) {
            return Err(ValidationError::DuplicateLaunchOptimization(
                option_id.clone(),
            ));
        }

        if !LAUNCH_OPTIMIZATION_DEFINITIONS
            .iter()
            .any(|definition| definition.id == option_id)
        {
            return Err(ValidationError::UnknownLaunchOptimization(option_id.clone()));
        }
    }

    let selected_ids = seen_ids;
    let mut directives = LaunchDirectives::default();

    for definition in LAUNCH_OPTIMIZATION_DEFINITIONS {
        if !selected_ids.contains(definition.id) {
            continue;
        }

        if definition.applies_to_method != resolved_method {
            return Err(ValidationError::LaunchOptimizationNotSupportedForMethod {
                option_id: definition.id.to_string(),
                method: resolved_method.to_string(),
            });
        }

        for conflicting_id in definition.conflicts_with {
            if selected_ids.contains(conflicting_id) {
                return Err(ValidationError::IncompatibleLaunchOptimizations {
                    first: definition.id.to_string(),
                    second: (*conflicting_id).to_string(),
                });
            }
        }

        if let Some(binary) = definition.required_binary {
            if !is_command_available(binary) {
                return Err(ValidationError::LaunchOptimizationDependencyMissing {
                    option_id: definition.id.to_string(),
                    dependency: binary.to_string(),
                });
            }
        }

        for (key, value) in definition.env {
            if !LAUNCH_OPTIMIZATION_ENV_VARS.contains(key) {
                return Err(ValidationError::UnknownLaunchOptimization(definition.id.to_string()));
            }

            directives.env.push(((*key).to_string(), (*value).to_string()));
        }

        for wrapper in definition.wrappers {
            directives.wrappers.push((*wrapper).to_string());
        }
    }

    Ok(directives)
}

fn is_command_available(binary: &str) -> bool {
    #[cfg(test)]
    {
        let guard = test_command_search_path()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(search_path) = guard.as_ref() {
            return is_executable_file(&search_path.join(binary));
        }
    }

    let Some(path_value) = env::var_os("PATH") else {
        return false;
    };

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
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    use crate::launch::request::{LaunchOptimizationsRequest, RuntimeLaunchConfig, SteamLaunchConfig};

    fn optimization_request(enabled_option_ids: Vec<&str>) -> LaunchRequest {
        LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/games/test.exe".to_string(),
            trainer_path: "/trainers/test.exe".to_string(),
            trainer_host_path: "/trainers/test.exe".to_string(),
            steam: SteamLaunchConfig::default(),
            runtime: RuntimeLaunchConfig {
                prefix_path: "/prefix".to_string(),
                proton_path: "/proton".to_string(),
                working_directory: String::new(),
            },
            optimizations: LaunchOptimizationsRequest {
                enabled_option_ids: enabled_option_ids
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            },
            launch_trainer_only: false,
            launch_game_only: false,
        }
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
        let request = optimization_request(vec!["enable_hdr", "disable_steam_input"]);

        let directives = resolve_launch_directives(&request).expect("resolve directives");

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
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mangohud_path = temp_dir.path().join("mangohud");
        let gamemode_path = temp_dir.path().join("gamemoderun");
        write_executable_file(&mangohud_path);
        write_executable_file(&gamemode_path);
        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let request = optimization_request(vec!["use_gamemode", "show_mangohud_overlay"]);
        let directives = resolve_launch_directives(&request).expect("resolve directives");

        assert_eq!(directives.wrappers, vec!["mangohud", "gamemoderun"]);
        assert!(directives.env.is_empty());
    }

    #[test]
    fn rejects_missing_wrapper_dependency() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let request = optimization_request(vec!["show_mangohud_overlay"]);
        let error = resolve_launch_directives(&request).expect_err("missing wrapper should fail");

        assert_eq!(
            error,
            ValidationError::LaunchOptimizationDependencyMissing {
                option_id: "show_mangohud_overlay".to_string(),
                dependency: "mangohud".to_string(),
            }
        );
    }
}
