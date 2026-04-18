use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use directories::BaseDirs;

use crate::launch::{runtime_helpers::build_gamescope_args, LaunchRequest, ValidationError};
use crate::profile::GamescopeConfig;

const TRAINER_HOST_EXPLICIT_ENV_KEYS: [&str; 19] = [
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "PATH",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "GAMESCOPE_WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "XAUTHORITY",
    "XDG_SESSION_TYPE",
    "XDG_CURRENT_DESKTOP",
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
    "WINEPREFIX",
    "GAMEID",
    "SteamGameId",
    "SteamAppId",
];

pub(super) fn prepare_gamescope_launch(
    config: &GamescopeConfig,
    wrappers: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut gamescope_args = build_gamescope_args(config);
    let has_mangohud = wrappers.iter().any(|w| w.trim() == "mangohud");
    let filtered_wrappers: Vec<String> = if has_mangohud {
        gamescope_args.push("--mangoapp".into());
        wrappers
            .iter()
            .filter(|w| w.trim() != "mangohud")
            .cloned()
            .collect()
    } else {
        wrappers.to_vec()
    };
    (gamescope_args, filtered_wrappers)
}

pub fn gamescope_pid_capture_path(log_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.gamescope.pid", log_path.to_string_lossy()))
}

pub(super) fn should_skip_gamescope(config: &GamescopeConfig) -> bool {
    !config.allow_nested && std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
}

pub(super) fn merge_mangohud_config_env_into_map(
    env: &mut BTreeMap<String, String>,
    request: &LaunchRequest,
    gamescope_active: bool,
    wrappers_had_mangohud: bool,
) {
    if !request.mangohud.enabled {
        return;
    }

    let user_overrode_configfile = request.custom_env_vars.contains_key("MANGOHUD_CONFIGFILE");

    if !user_overrode_configfile {
        let profile_name = match request.profile_name.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => {
                tracing::warn!(
                    "mangohud config enabled but profile_name is missing in LaunchRequest; \
                     skipping MANGOHUD_CONFIGFILE injection"
                );
                if gamescope_active && wrappers_had_mangohud {
                    env.insert("MANGOHUD_CONFIG".to_string(), "read_cfg".to_string());
                }
                return;
            }
        };

        let base_path = match BaseDirs::new() {
            Some(dirs) => dirs.config_dir().join("crosshook").join("profiles"),
            None => {
                tracing::warn!(
                    "mangohud config enabled but home directory could not be resolved; \
                     skipping MANGOHUD_CONFIGFILE injection"
                );
                if gamescope_active && wrappers_had_mangohud {
                    env.insert("MANGOHUD_CONFIG".to_string(), "read_cfg".to_string());
                }
                return;
            }
        };

        let conf_path = crate::profile::mangohud::mangohud_conf_path(&base_path, profile_name);

        if conf_path.is_file() {
            env.insert(
                "MANGOHUD_CONFIGFILE".to_string(),
                conf_path.to_string_lossy().into_owned(),
            );
        } else {
            tracing::warn!(
                "mangohud config file not found at {}; skipping MANGOHUD_CONFIGFILE injection",
                conf_path.display()
            );
        }
    }

    if gamescope_active && wrappers_had_mangohud {
        env.insert("MANGOHUD_CONFIG".to_string(), "read_cfg".to_string());
    }
}

pub(super) fn insert_sorted_env_key_list(
    env: &mut BTreeMap<String, String>,
    name: &str,
    keys: impl IntoIterator<Item = String>,
) {
    let mut keys = keys
        .into_iter()
        .filter(|key| !key.trim().is_empty())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        env.remove(name);
        return;
    }
    keys.sort_unstable();
    keys.dedup();
    env.insert(name.to_string(), keys.join(","));
}

pub(super) fn collect_trainer_builtin_env_keys(
    env: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
) -> Vec<String> {
    env.keys()
        .filter(|key| !TRAINER_HOST_EXPLICIT_ENV_KEYS.contains(&key.as_str()))
        .filter(|key| !custom_env_vars.contains_key(*key))
        .filter(|key| !key.starts_with("CROSSHOOK_TRAINER_"))
        .cloned()
        .collect()
}

pub(super) fn validation_error_to_io_error(error: ValidationError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
}
