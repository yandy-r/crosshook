use std::path::Path;

use directories::BaseDirs;

use super::types::{EnvVarSource, PreviewEnvVar};
use crate::launch::optimizations::LaunchDirectives;
use crate::launch::request::LaunchRequest;
use crate::launch::runtime_helpers::{
    collect_pressure_vessel_paths, env_value, resolve_proton_paths,
    resolve_steam_client_install_path, DEFAULT_HOST_PATH,
};
use crate::launch::script_runner::{
    force_no_umu_for_launch_request, proton_path_dirname, resolve_launch_proton_path,
    should_use_umu,
};

/// Collects host environment variables that will be passed through to the launch command.
pub(super) fn collect_host_environment(env: &mut Vec<PreviewEnvVar>) {
    const DEFAULT_SHELL: &str = "/bin/bash";
    let host_vars: &[(&str, &str)] = &[
        ("HOME", ""),
        ("USER", ""),
        ("LOGNAME", ""),
        ("SHELL", DEFAULT_SHELL),
        ("PATH", DEFAULT_HOST_PATH),
        ("DISPLAY", ""),
        ("WAYLAND_DISPLAY", ""),
        ("XDG_RUNTIME_DIR", ""),
        ("DBUS_SESSION_BUS_ADDRESS", ""),
    ];

    for (key, default) in host_vars {
        env.push(PreviewEnvVar {
            key: key.to_string(),
            value: env_value(key, default),
            source: EnvVarSource::Host,
        });
    }
}

/// Collects Proton runtime environment variables for `proton_run` launches.
///
/// Uses `resolve_wine_prefix_path()` heuristic for WINEPREFIX resolution,
/// which differs from `steam_applaunch` (hardcoded `{compatdata}/pfx`).
pub(super) fn collect_runtime_proton_environment(
    request: &LaunchRequest,
    env: &mut Vec<PreviewEnvVar>,
) {
    let resolved_paths = resolve_proton_paths(Path::new(request.runtime.prefix_path.trim()));

    env.push(PreviewEnvVar {
        key: "WINEPREFIX".to_string(),
        value: resolved_paths
            .wine_prefix_path
            .to_string_lossy()
            .into_owned(),
        source: EnvVarSource::ProtonRuntime,
    });

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_DATA_PATH".to_string(),
        value: resolved_paths
            .compat_data_path
            .to_string_lossy()
            .into_owned(),
        source: EnvVarSource::ProtonRuntime,
    });

    if let Some(steam_client_path) =
        resolve_steam_client_install_path(request.steam.steam_client_install_path.trim())
    {
        env.push(PreviewEnvVar {
            key: "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            value: steam_client_path,
            source: EnvVarSource::ProtonRuntime,
        });
    }

    let proton_verb = if request.launch_trainer_only {
        "runinprefix"
    } else {
        "waitforexitandrun"
    };
    env.push(PreviewEnvVar {
        key: "PROTON_VERB".to_string(),
        value: proton_verb.to_string(),
        source: EnvVarSource::ProtonRuntime,
    });

    let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":");
    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_LIBRARY_PATHS".to_string(),
        value: pressure_vessel_paths.clone(),
        source: EnvVarSource::ProtonRuntime,
    });
    env.push(PreviewEnvVar {
        key: "PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(),
        value: pressure_vessel_paths,
        source: EnvVarSource::ProtonRuntime,
    });

    let force_no_umu = force_no_umu_for_launch_request(request);
    let (use_umu, _umu_run_path) = should_use_umu(request, force_no_umu);
    if use_umu {
        let resolved = resolve_launch_proton_path(
            request.runtime.proton_path.trim(),
            request.steam.steam_client_install_path.trim(),
        );
        let dirname = proton_path_dirname(resolved.trim());
        env.push(PreviewEnvVar {
            key: "PROTONPATH".to_string(),
            value: dirname,
            source: EnvVarSource::ProtonRuntime,
        });
    }
}

/// Collects Steam-specific Proton environment variables for `steam_applaunch` launches.
///
/// Uses hardcoded `{compatdata}/pfx` for WINEPREFIX, NOT `resolve_wine_prefix_path()`.
pub(super) fn collect_steam_proton_environment(
    request: &LaunchRequest,
    env: &mut Vec<PreviewEnvVar>,
) {
    let compatdata = request.steam.compatdata_path.trim();

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_DATA_PATH".to_string(),
        value: compatdata.to_string(),
        source: EnvVarSource::SteamProton,
    });

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
        value: request.steam.steam_client_install_path.trim().to_string(),
        source: EnvVarSource::SteamProton,
    });

    env.push(PreviewEnvVar {
        key: "WINEPREFIX".to_string(),
        value: Path::new(compatdata)
            .join("pfx")
            .to_string_lossy()
            .into_owned(),
        source: EnvVarSource::SteamProton,
    });
}

pub(super) fn merge_optimization_and_custom_preview_env(
    request: &LaunchRequest,
    directives: &LaunchDirectives,
    env: &mut Vec<PreviewEnvVar>,
) {
    for (key, value) in &directives.env {
        upsert_preview_env(env, key, value, EnvVarSource::LaunchOptimization);
    }
    for (key, value) in &request.custom_env_vars {
        upsert_preview_env(env, key, value, EnvVarSource::ProfileCustom);
    }
}

pub(super) fn merge_custom_preview_env_only(request: &LaunchRequest, env: &mut Vec<PreviewEnvVar>) {
    for (key, value) in &request.custom_env_vars {
        upsert_preview_env(env, key, value, EnvVarSource::ProfileCustom);
    }
}

pub(super) fn upsert_preview_env(
    env: &mut Vec<PreviewEnvVar>,
    key: &str,
    value: &str,
    source: EnvVarSource,
) {
    if let Some(existing) = env.iter_mut().find(|e| e.key == key) {
        existing.value = value.to_string();
        existing.source = source;
    } else {
        env.push(PreviewEnvVar {
            key: key.to_string(),
            value: value.to_string(),
            source,
        });
    }
}

/// Inserts a preview env var only if the key is not already present.
pub(super) fn insert_preview_env_if_absent(
    env: &mut Vec<PreviewEnvVar>,
    key: &str,
    value: &str,
    source: EnvVarSource,
) {
    if !env.iter().any(|e| e.key == key) {
        env.push(PreviewEnvVar {
            key: key.to_string(),
            value: value.to_string(),
            source,
        });
    }
}

/// Injects `MANGOHUD_CONFIGFILE` (and optionally `MANGOHUD_CONFIG=read_cfg`) into the preview
/// environment vars when the profile has MangoHud config enabled.
///
/// Respects user-supplied `MANGOHUD_CONFIGFILE` in `custom_env_vars` by skipping injection when
/// the key is already present.  The preview path does not check whether the config file exists on
/// disk — it shows what *would* be set.
pub(super) fn inject_mangohud_config_preview_env(
    env: &mut Vec<PreviewEnvVar>,
    request: &LaunchRequest,
    gamescope_active: bool,
    wrappers_had_mangohud: bool,
) {
    fn ensure_mangohud_read_cfg(
        env: &mut Vec<PreviewEnvVar>,
        gamescope_active: bool,
        wrappers_had_mangohud: bool,
    ) {
        if gamescope_active && wrappers_had_mangohud {
            insert_preview_env_if_absent(
                env,
                "MANGOHUD_CONFIG",
                "read_cfg",
                EnvVarSource::ProfileCustom,
            );
        }
    }

    if !request.mangohud.enabled {
        return;
    }

    let user_overrode_configfile = request.custom_env_vars.contains_key("MANGOHUD_CONFIGFILE");

    // Inject MANGOHUD_CONFIGFILE only when the user hasn't explicitly set it.
    if !user_overrode_configfile {
        let profile_name = match request.profile_name.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => {
                // Still fall through to set read_cfg below if gamescope is active.
                ensure_mangohud_read_cfg(env, gamescope_active, wrappers_had_mangohud);
                return;
            }
        };

        let base_path = match BaseDirs::new() {
            Some(dirs) => dirs.config_dir().join("crosshook").join("profiles"),
            None => {
                ensure_mangohud_read_cfg(env, gamescope_active, wrappers_had_mangohud);
                return;
            }
        };

        let conf_path = crate::profile::mangohud::mangohud_conf_path(&base_path, profile_name);
        let conf_path_str = conf_path.to_string_lossy().into_owned();

        insert_preview_env_if_absent(
            env,
            "MANGOHUD_CONFIGFILE",
            &conf_path_str,
            EnvVarSource::ProfileCustom,
        );
    }

    // Always set read_cfg for gamescope compatibility, regardless of who supplied MANGOHUD_CONFIGFILE.
    ensure_mangohud_read_cfg(env, gamescope_active, wrappers_had_mangohud);
}
