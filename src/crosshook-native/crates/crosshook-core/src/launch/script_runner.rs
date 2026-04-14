use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use tokio::process::Command;

use super::{
    resolve_launch_directives, resolve_launch_directives_for_method,
    runtime_helpers::{
        apply_working_directory, attach_log_stdio,
        build_direct_proton_command_with_wrappers_in_directory, build_gamescope_args,
        build_proton_command_with_gamescope_in_directory,
        build_proton_command_with_gamescope_pid_capture_in_directory, host_environment_map,
        merge_optimization_and_custom_into_map, merge_runtime_proton_into_map,
        resolve_effective_working_directory, resolve_wine_prefix_path,
    },
    LaunchRequest, ValidationError, METHOD_PROTON_RUN,
};
use crate::platform::{self, host_command_with_env, normalize_flatpak_host_path};
use crate::profile::{GamescopeConfig, TrainerLoadingMode};
use crate::steam::{discover_steam_root_candidates, proton::prefer_user_local_compat_tool_path};

const BASH_EXECUTABLE: &str = "/bin/bash";
const DEFAULT_GAME_STARTUP_DELAY_SECONDS: &str = "30";
const DEFAULT_GAME_TIMEOUT_SECONDS: &str = "90";
const DEFAULT_TRAINER_TIMEOUT_SECONDS: &str = "10";
const STAGED_TRAINER_ROOT: &str = "CrossHook/StagedTrainers";
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
const SUPPORT_DIRECTORIES: [&str; 9] = [
    "assets",
    "data",
    "lib",
    "bin",
    "runtimes",
    "plugins",
    "locales",
    "cef",
    "resources",
];
const SHARED_DEPENDENCY_EXTENSIONS: [&str; 7] =
    ["dll", "json", "config", "ini", "pak", "dat", "bin"];

fn prepare_gamescope_launch(
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

fn should_skip_gamescope(config: &GamescopeConfig) -> bool {
    !config.allow_nested && std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
}

fn build_flatpak_unshare_bash_command(
    script_path: &Path,
    env: &BTreeMap<String, String>,
    custom_env: &BTreeMap<String, String>,
) -> std::io::Result<Command> {
    let mut base = env.clone();
    for key in custom_env.keys() {
        base.remove(key);
    }
    // Flatpak host spawns run with --clear-env; forward session variables needed by
    // display/compositor and user-session integration so helper behavior matches native/AppImage.
    for key in [
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
    ] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                base.entry(key.to_string()).or_insert(value);
            }
        }
    }
    let mut cmd = host_command_with_env("unshare", &base, custom_env);
    cmd.args(["--net", BASH_EXECUTABLE]);
    cmd.arg(script_path);
    Ok(cmd)
}

/// Injects `MANGOHUD_CONFIGFILE` (and optionally `MANGOHUD_CONFIG=read_cfg`) into a command when
/// the profile has MangoHud config enabled.
///
/// Skips injection if the user has already set `MANGOHUD_CONFIGFILE` in `custom_env_vars` so that
/// explicit user overrides are respected.
///
/// When both gamescope and a mangohud wrapper are active, also sets `MANGOHUD_CONFIG=read_cfg`
/// for older gamescope compatibility.
fn merge_mangohud_config_env_into_map(
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

fn insert_sorted_env_key_list(
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

fn collect_trainer_builtin_env_keys(
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

pub fn build_helper_command(
    request: &LaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> std::io::Result<Command> {
    let directives = resolve_launch_directives_for_method(
        &request.optimizations.enabled_option_ids,
        METHOD_PROTON_RUN,
    )
    .map_err(validation_error_to_io_error)?;
    let mut env = host_environment_map();
    merge_steam_helper_env_into(&mut env, request);
    merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
    let trainer_gamescope = request.resolved_trainer_gamescope();
    let trainer_gamescope_active =
        trainer_gamescope.enabled && !should_skip_gamescope(&trainer_gamescope);
    let trainer_wrappers_had_mangohud = directives.wrappers.iter().any(|w| w.trim() == "mangohud");
    merge_mangohud_config_env_into_map(
        &mut env,
        request,
        trainer_gamescope_active,
        trainer_wrappers_had_mangohud,
    );
    let builtin_trainer_env_keys = collect_trainer_builtin_env_keys(&env, &request.custom_env_vars);
    insert_sorted_env_key_list(
        &mut env,
        "CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS",
        builtin_trainer_env_keys,
    );
    insert_sorted_env_key_list(
        &mut env,
        "CROSSHOOK_TRAINER_CUSTOM_ENV_KEYS",
        request.custom_env_vars.keys().cloned(),
    );
    let mut base_for_flatpak = env.clone();
    for key in request.custom_env_vars.keys() {
        base_for_flatpak.remove(key);
    }
    let resolved_proton_path = resolve_launch_proton_path(
        request.steam.proton_path.as_str(),
        request.steam.steam_client_install_path.as_str(),
    );
    let normalized_steam_client_install_path =
        normalize_flatpak_host_path(&request.steam.steam_client_install_path);

    tracing::debug!(
        configured_proton_path = request.steam.proton_path.trim(),
        resolved_proton_path = resolved_proton_path.trim(),
        steam_client_install_path = normalized_steam_client_install_path.trim(),
        helper_script = %script_path.display(),
        launch_game_only = request.launch_game_only,
        launch_trainer_only = request.launch_trainer_only,
        "building steam helper launch"
    );

    let mut command = if request.network_isolation
        && !request.launch_game_only
        && super::runtime_helpers::is_unshare_net_available()
    {
        if platform::is_flatpak() {
            build_flatpak_unshare_bash_command(
                script_path,
                &base_for_flatpak,
                &request.custom_env_vars,
            )?
        } else {
            merge_optimization_and_custom_into_map(
                &mut env,
                &directives.env,
                &request.custom_env_vars,
            );
            let mut cmd = Command::new("unshare");
            cmd.envs(&env);
            cmd.args(["--net", BASH_EXECUTABLE]);
            cmd.arg(script_path);
            cmd
        }
    } else {
        merge_optimization_and_custom_into_map(&mut env, &directives.env, &request.custom_env_vars);
        let mut cmd = Command::new(BASH_EXECUTABLE);
        cmd.arg(script_path);
        cmd.envs(&env);
        cmd
    };
    command.args(helper_arguments(
        request,
        log_path,
        resolved_proton_path.trim(),
        normalized_steam_client_install_path.trim(),
        &trainer_gamescope,
    ));
    Ok(command)
}

// Non-Flatpak Steam trainer-only helper path.
//
// This function intentionally does NOT emit CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS or
// CROSSHOOK_TRAINER_CUSTOM_ENV_KEYS. Per the CLAUDE.md trainer execution parity rule,
// the trainer-only Steam helper path aligns with `proton_run` semantics and drops the
// optimization env that the combined game+trainer `build_helper_command` path emits.
//
// Downstream runner scripts (e.g. steam-host-trainer-runner.sh:441-442) call
// `capture_preserved_trainer_env` which reads those vars; they handle missing or empty
// CROSSHOOK_TRAINER_*_ENV_KEYS gracefully (treating them as empty). This is intentional
// and documented here to avoid future confusion during env-inheritance debugging under
// `steam_applaunch` trainer-only runs. See issue #229.
pub fn build_trainer_command(
    request: &LaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> std::io::Result<Command> {
    let mut env = host_environment_map();
    merge_steam_helper_env_into(&mut env, request);
    let base_for_flatpak = env.clone();
    let resolved_proton_path = resolve_launch_proton_path(
        request.steam.proton_path.as_str(),
        request.steam.steam_client_install_path.as_str(),
    );
    let normalized_steam_client_install_path =
        normalize_flatpak_host_path(&request.steam.steam_client_install_path);
    let trainer_gamescope = request.resolved_trainer_gamescope();

    tracing::debug!(
        configured_proton_path = request.steam.proton_path.trim(),
        resolved_proton_path = resolved_proton_path.trim(),
        steam_client_install_path = normalized_steam_client_install_path.trim(),
        helper_script = %script_path.display(),
        trainer_loading_mode = request.trainer_loading_mode.as_str(),
        "building steam trainer helper launch"
    );

    let mut command = if request.network_isolation
        && super::runtime_helpers::is_unshare_net_available()
    {
        if platform::is_flatpak() {
            build_flatpak_unshare_bash_command(script_path, &base_for_flatpak, &BTreeMap::new())?
        } else {
            let mut cmd = Command::new("unshare");
            cmd.envs(&env);
            cmd.args(["--net", BASH_EXECUTABLE]);
            cmd.arg(script_path);
            cmd
        }
    } else {
        let mut cmd = Command::new(BASH_EXECUTABLE);
        cmd.arg(script_path);
        cmd.envs(&env);
        cmd
    };
    command.args(trainer_arguments(
        request,
        log_path,
        resolved_proton_path.trim(),
        normalized_steam_client_install_path.trim(),
        &trainer_gamescope,
    ));
    Ok(command)
}

pub fn build_flatpak_steam_trainer_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let mut direct_request = request.clone();
    direct_request.method = METHOD_PROTON_RUN.to_string();
    direct_request.runtime.prefix_path =
        normalize_flatpak_host_path(&request.steam.compatdata_path);
    direct_request.runtime.proton_path = request.steam.proton_path.clone();

    build_proton_trainer_command(&direct_request, log_path)
}

pub fn build_proton_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let directives = resolve_launch_directives(request).map_err(validation_error_to_io_error)?;
    let gamescope_active = request.gamescope.enabled && !should_skip_gamescope(&request.gamescope);
    let wrappers_had_mangohud = directives.wrappers.iter().any(|w| w.trim() == "mangohud");

    let mut env = host_environment_map();
    merge_runtime_proton_into_map(
        &mut env,
        request.runtime.prefix_path.trim(),
        request.steam.steam_client_install_path.trim(),
    );
    merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
    env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
    merge_mangohud_config_env_into_map(&mut env, request, gamescope_active, wrappers_had_mangohud);
    for key in request.custom_env_vars.keys() {
        env.remove(key);
    }

    let normalized_game_path = normalize_flatpak_host_path(&request.game_path);
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let effective_working_directory = resolve_effective_working_directory(
        normalized_working_directory.trim(),
        Path::new(normalized_game_path.trim()),
    );
    let resolved_proton_path = resolve_launch_proton_path(
        request.runtime.proton_path.trim(),
        request.steam.steam_client_install_path.trim(),
    );
    let resolved_steam_client_install_path = env
        .get("STEAM_COMPAT_CLIENT_INSTALL_PATH")
        .map(String::as_str)
        .unwrap_or("");

    tracing::debug!(
        configured_proton_path = request.runtime.proton_path.trim(),
        resolved_proton_path = resolved_proton_path.trim(),
        steam_client_install_path = resolved_steam_client_install_path,
        target_path = normalized_game_path.trim(),
        working_directory = effective_working_directory.as_deref().unwrap_or(""),
        gamescope_active,
        wrapper_count = directives.wrappers.len(),
        "building proton game launch"
    );

    let mut command = if gamescope_active {
        let (gamescope_args, filtered_wrappers) =
            prepare_gamescope_launch(&request.gamescope, &directives.wrappers);
        if platform::is_flatpak() {
            let pid_capture_path = gamescope_pid_capture_path(log_path);
            build_proton_command_with_gamescope_pid_capture_in_directory(
                resolved_proton_path.as_str(),
                &filtered_wrappers,
                &gamescope_args,
                &env,
                effective_working_directory.as_deref(),
                &request.custom_env_vars,
                Some(&pid_capture_path),
            )
        } else {
            build_proton_command_with_gamescope_in_directory(
                resolved_proton_path.as_str(),
                &filtered_wrappers,
                &gamescope_args,
                &env,
                effective_working_directory.as_deref(),
                &request.custom_env_vars,
            )
        }
    } else {
        build_direct_proton_command_with_wrappers_in_directory(
            resolved_proton_path.as_str(),
            &directives.wrappers,
            &env,
            effective_working_directory.as_deref(),
            &request.custom_env_vars,
        )
    };
    command.arg(normalized_game_path.trim());
    Ok(command)
}

pub fn build_proton_trainer_command(
    request: &LaunchRequest,
    _log_path: &Path,
) -> std::io::Result<Command> {
    let normalized_prefix_path = normalize_flatpak_host_path(&request.runtime.prefix_path);
    let normalized_trainer_host_path = normalize_flatpak_host_path(&request.trainer_host_path);
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let effective_working_directory = resolve_effective_working_directory(
        normalized_working_directory.trim(),
        Path::new(normalized_trainer_host_path.trim()),
    );
    let trainer_launch_path = match request.trainer_loading_mode {
        TrainerLoadingMode::SourceDirectory => normalized_trainer_host_path.trim().to_string(),
        TrainerLoadingMode::CopyToPrefix => stage_trainer_into_prefix(
            Path::new(normalized_prefix_path.trim()),
            Path::new(normalized_trainer_host_path.trim()),
        )?,
    };

    let effective_wrappers =
        if request.network_isolation && super::runtime_helpers::is_unshare_net_available() {
            vec!["unshare".to_string(), "--net".to_string()]
        } else {
            Vec::new()
        };

    let trainer_gamescope = request.resolved_trainer_gamescope();
    let gamescope_active = trainer_gamescope.enabled && !should_skip_gamescope(&trainer_gamescope);

    let mut env = host_environment_map();
    merge_runtime_proton_into_map(
        &mut env,
        request.runtime.prefix_path.trim(),
        request.steam.steam_client_install_path.trim(),
    );
    env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
    let resolved_proton_path = resolve_launch_proton_path(
        request.runtime.proton_path.trim(),
        request.steam.steam_client_install_path.trim(),
    );
    let resolved_steam_client_install_path = env
        .get("STEAM_COMPAT_CLIENT_INSTALL_PATH")
        .map(String::as_str)
        .unwrap_or("");
    tracing::debug!(
        configured_proton_path = request.runtime.proton_path.trim(),
        resolved_proton_path = resolved_proton_path.trim(),
        steam_client_install_path = resolved_steam_client_install_path,
        target_path = trainer_launch_path.as_str(),
        trainer_host_path = normalized_trainer_host_path.trim(),
        working_directory = effective_working_directory.as_deref().unwrap_or(""),
        gamescope_active,
        wrapper_count = effective_wrappers.len(),
        trainer_loading_mode = request.trainer_loading_mode.as_str(),
        "building proton trainer launch"
    );

    let mut command = if gamescope_active {
        let (gamescope_args, filtered_wrappers) =
            prepare_gamescope_launch(&trainer_gamescope, &effective_wrappers);
        build_proton_command_with_gamescope_in_directory(
            resolved_proton_path.as_str(),
            &filtered_wrappers,
            &gamescope_args,
            &env,
            effective_working_directory.as_deref(),
            &BTreeMap::new(),
        )
    } else {
        build_direct_proton_command_with_wrappers_in_directory(
            resolved_proton_path.as_str(),
            &effective_wrappers,
            &env,
            effective_working_directory.as_deref(),
            &BTreeMap::new(),
        )
    };
    command.arg(trainer_launch_path);
    Ok(command)
}

pub fn build_native_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let normalized_game_path = normalize_flatpak_host_path(&request.game_path);
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let mut env = host_environment_map();
    merge_optimization_and_custom_into_map(&mut env, &[], &request.custom_env_vars);
    let mut command = Command::new(normalized_game_path.trim());
    command.envs(&env);
    apply_working_directory(
        &mut command,
        normalized_working_directory.trim(),
        Path::new(normalized_game_path.trim()),
    );
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}

fn merge_steam_helper_env_into(map: &mut BTreeMap<String, String>, request: &LaunchRequest) {
    let normalized_compatdata_path = normalize_flatpak_host_path(&request.steam.compatdata_path);
    let normalized_steam_client_install_path =
        normalize_flatpak_host_path(&request.steam.steam_client_install_path);
    map.insert(
        "STEAM_COMPAT_DATA_PATH".to_string(),
        normalized_compatdata_path.trim().to_string(),
    );
    map.insert(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
        normalized_steam_client_install_path.trim().to_string(),
    );
    map.insert(
        "WINEPREFIX".to_string(),
        Path::new(normalized_compatdata_path.trim())
            .join("pfx")
            .to_string_lossy()
            .into_owned(),
    );
}

fn resolve_launch_proton_path(proton_path: &str, steam_client_install_path: &str) -> String {
    resolve_launch_proton_path_with_mode(
        proton_path,
        steam_client_install_path,
        platform::is_flatpak(),
    )
}

fn resolve_launch_proton_path_with_mode(
    proton_path: &str,
    steam_client_install_path: &str,
    flatpak: bool,
) -> String {
    let normalized_proton_path = normalize_flatpak_host_path(proton_path);
    let trimmed_proton_path = normalized_proton_path.trim();
    if trimmed_proton_path.is_empty() || !flatpak {
        return normalized_proton_path;
    }

    let configured_steam_client_install_path =
        normalize_flatpak_host_path(steam_client_install_path);
    let mut diagnostics = Vec::new();
    let steam_root_candidates = discover_steam_root_candidates(
        configured_steam_client_install_path.as_str(),
        &mut diagnostics,
    );
    let resolved_path = prefer_user_local_compat_tool_path(
        Path::new(trimmed_proton_path),
        &steam_root_candidates,
        &mut diagnostics,
    );

    for entry in diagnostics {
        tracing::debug!(entry, "proton launch resolution diagnostic");
    }

    resolved_path.to_string_lossy().into_owned()
}

fn helper_arguments(
    request: &LaunchRequest,
    log_path: &Path,
    resolved_proton_path: &str,
    normalized_steam_client_install_path: &str,
    trainer_gamescope: &GamescopeConfig,
) -> Vec<OsString> {
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let mut arguments = vec![
        "--appid".to_string().into(),
        request.steam.app_id.clone().into(),
        "--compatdata".to_string().into(),
        normalize_flatpak_host_path(&request.steam.compatdata_path).into(),
        "--proton".into(),
        resolved_proton_path.to_string().into(),
        "--steam-client".into(),
        normalized_steam_client_install_path.to_string().into(),
        "--game-exe-name".into(),
        request.game_executable_name().into(),
        "--trainer-path".into(),
        request.trainer_path.clone().into(),
        "--trainer-host-path".into(),
        normalize_flatpak_host_path(&request.trainer_host_path).into(),
        "--trainer-loading-mode".into(),
        request.trainer_loading_mode.as_str().to_string().into(),
        "--log-file".into(),
        log_path.as_os_str().to_owned(),
    ];

    if !normalized_working_directory.trim().is_empty() {
        arguments.push("--working-directory".into());
        arguments.push(normalized_working_directory.into());
    }

    if trainer_gamescope.enabled && !should_skip_gamescope(trainer_gamescope) {
        arguments.push("--gamescope-enabled".into());
        if trainer_gamescope.allow_nested {
            arguments.push("--gamescope-allow-nested".into());
        }
        for arg in build_gamescope_args(trainer_gamescope) {
            arguments.push("--gamescope-arg".into());
            arguments.push(arg.into());
        }
    }

    arguments.extend([
        "--game-startup-delay-seconds".into(),
        DEFAULT_GAME_STARTUP_DELAY_SECONDS.into(),
        "--game-timeout-seconds".into(),
        DEFAULT_GAME_TIMEOUT_SECONDS.into(),
        "--trainer-timeout-seconds".into(),
        DEFAULT_TRAINER_TIMEOUT_SECONDS.into(),
    ]);

    if request.launch_trainer_only {
        arguments.push("--trainer-only".into());
    }

    if request.launch_game_only {
        arguments.push("--game-only".into());
    }

    arguments
}

fn trainer_arguments(
    request: &LaunchRequest,
    log_path: &Path,
    resolved_proton_path: &str,
    normalized_steam_client_install_path: &str,
    trainer_gamescope: &GamescopeConfig,
) -> Vec<OsString> {
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let mut arguments = vec![
        "--compatdata".into(),
        normalize_flatpak_host_path(&request.steam.compatdata_path).into(),
        "--proton".into(),
        resolved_proton_path.to_string().into(),
        "--steam-client".into(),
        normalized_steam_client_install_path.to_string().into(),
        "--steam-app-id".into(),
        request.steam.app_id.clone().into(),
        "--trainer-path".into(),
        request.trainer_path.clone().into(),
        "--trainer-host-path".into(),
        normalize_flatpak_host_path(&request.trainer_host_path).into(),
        "--trainer-loading-mode".into(),
        request.trainer_loading_mode.as_str().to_string().into(),
        "--log-file".into(),
        log_path.as_os_str().to_owned(),
    ];

    if !normalized_working_directory.trim().is_empty() {
        arguments.push("--working-directory".into());
        arguments.push(normalized_working_directory.into());
    }

    if trainer_gamescope.enabled && !should_skip_gamescope(trainer_gamescope) {
        arguments.push("--gamescope-enabled".into());
        if trainer_gamescope.allow_nested {
            arguments.push("--gamescope-allow-nested".into());
        }
        for arg in build_gamescope_args(trainer_gamescope) {
            arguments.push("--gamescope-arg".into());
            arguments.push(arg.into());
        }
    }

    arguments
}

fn stage_trainer_into_prefix(
    prefix_path: &Path,
    trainer_host_path: &Path,
) -> std::io::Result<String> {
    let trainer_file_name = trainer_host_path
        .file_name()
        .ok_or_else(|| io_error("trainer host path is missing a file name"))?;
    let trainer_base_name = trainer_host_path
        .file_stem()
        .ok_or_else(|| io_error("trainer host path is missing a file stem"))?;
    let trainer_source_dir = trainer_host_path
        .parent()
        .ok_or_else(|| io_error("trainer host path is missing a parent directory"))?;

    let wine_prefix_path = resolve_wine_prefix_path(prefix_path);
    let staged_root = wine_prefix_path
        .join("drive_c")
        .join(PathBuf::from(STAGED_TRAINER_ROOT));
    let staged_directory = staged_root.join(trainer_base_name);
    let staged_host_path = staged_directory.join(trainer_file_name);

    if staged_directory.exists() {
        fs::remove_dir_all(&staged_directory)?;
    }

    fs::create_dir_all(&staged_directory)?;
    fs::copy(trainer_host_path, &staged_host_path)?;
    stage_trainer_support_files(
        trainer_source_dir,
        &staged_directory,
        trainer_file_name,
        trainer_base_name.to_string_lossy().as_ref(),
    )?;

    Ok(format!(
        "C:\\CrossHook\\StagedTrainers\\{}\\{}",
        trainer_base_name.to_string_lossy(),
        trainer_file_name.to_string_lossy()
    ))
}

fn stage_trainer_support_files(
    trainer_source_dir: &Path,
    staged_target_dir: &Path,
    trainer_file_name: &std::ffi::OsStr,
    trainer_base_name: &str,
) -> std::io::Result<()> {
    for entry in fs::read_dir(trainer_source_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();

        if file_name == trainer_file_name {
            continue;
        }

        if path.is_file() && should_stage_support_file(&file_name, trainer_base_name) {
            fs::copy(&path, staged_target_dir.join(&file_name))?;
        }
    }

    for directory in SUPPORT_DIRECTORIES {
        let source = trainer_source_dir.join(directory);
        if source.is_dir() {
            copy_dir_all(&source, &staged_target_dir.join(directory))?;
        }
    }

    Ok(())
}

/// Stages any sibling file with a recognized support extension (.dll, .ini, etc.).
/// The trainer executable itself is excluded by the caller before this check.
fn should_stage_support_file(file_name: &std::ffi::OsStr, _trainer_base_name: &str) -> bool {
    let file_name = file_name.to_string_lossy();
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();

    SHARED_DEPENDENCY_EXTENSIONS.contains(&extension.as_str())
}

fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_symlink() {
            tracing::debug!(path = %source_path.display(), "skipping symlink during trainer staging");
            continue;
        }

        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

fn io_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}

fn validation_error_to_io_error(error: ValidationError) -> std::io::Error {
    io_error(&error.to_string())
}

/// Returns the best available Steam App ID for umu-run's `GAMEID`.
/// Prefers `steam.app_id`, falls back to `runtime.steam_app_id`, then `""`.
fn resolve_steam_app_id_for_umu(request: &LaunchRequest) -> &str {
    let steam_id = request.steam.app_id.trim();
    if !steam_id.is_empty() {
        return steam_id;
    }
    let runtime_id = request.runtime.steam_app_id.trim();
    if !runtime_id.is_empty() {
        return runtime_id;
    }
    ""
}

fn resolved_umu_game_id_for_env(request: &LaunchRequest) -> String {
    let trimmed = resolve_steam_app_id_for_umu(request).trim();
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

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

    fn steam_request() -> LaunchRequest {
        LaunchRequest {
            method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
            game_path: "/games/My Game/game.exe".to_string(),
            trainer_path: "/trainers/trainer.exe".to_string(),
            trainer_host_path: "/trainers/trainer.exe".to_string(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig {
                app_id: "12345".to_string(),
                compatdata_path: "/tmp/compat".to_string(),
                proton_path: "/tmp/proton".to_string(),
                steam_client_install_path: "/tmp/steam".to_string(),
            },
            runtime: crate::launch::RuntimeLaunchConfig::default(),
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: false,
            launch_game_only: true,
            profile_name: None,
            ..Default::default()
        }
    }

    fn write_steam_client_root(path: &Path) {
        fs::create_dir_all(path.join("steamapps")).expect("steamapps dir");
        fs::create_dir_all(path.join("config")).expect("config dir");
    }

    fn command_env_value(command: &Command, key: &str) -> Option<String> {
        command
            .as_std()
            .get_envs()
            .find_map(|(env_key, env_value)| {
                (env_key == std::ffi::OsStr::new(key))
                    .then(|| env_value.map(|value| value.to_string_lossy().into_owned()))
            })
            .flatten()
    }

    #[test]
    fn proton_game_command_applies_optimization_wrappers_and_env() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let wrapper_dir = temp_dir.path().join("wrappers");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let game_path = temp_dir.path().join("game.exe");
        let log_path = temp_dir.path().join("game.log");
        let steam_client_path = temp_dir.path().join("steam-client");
        let workspace_dir = prefix_path.join("workspace");

        fs::create_dir_all(&wrapper_dir).expect("wrapper dir");
        fs::create_dir_all(&prefix_path).expect("prefix dir");
        fs::create_dir_all(&workspace_dir).expect("workspace dir");
        fs::create_dir_all(&steam_client_path).expect("steam client dir");
        write_executable_file(&wrapper_dir.join("mangohud"));
        write_executable_file(&wrapper_dir.join("gamemoderun"));
        write_executable_file(&proton_path);
        fs::write(&game_path, b"game").expect("game exe");

        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(&wrapper_dir);
        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: game_path.to_string_lossy().into_owned(),
            trainer_path: String::new(),
            trainer_host_path: String::new(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig {
                app_id: String::new(),
                compatdata_path: String::new(),
                proton_path: String::new(),
                steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
            },
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: workspace_dir.to_string_lossy().into_owned(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest {
                enabled_option_ids: vec![
                    "show_mangohud_overlay".to_string(),
                    "use_gamemode".to_string(),
                    "disable_steam_input".to_string(),
                ],
            },
            launch_trainer_only: false,
            launch_game_only: true,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_game_command(&request, &log_path).expect("game command");

        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(command.as_std().get_program().to_string_lossy(), "mangohud");
        assert_eq!(
            args,
            vec![
                "gamemoderun".to_string(),
                proton_path.to_string_lossy().into_owned(),
                "run".to_string(),
                game_path.to_string_lossy().into_owned(),
            ]
        );
        assert_eq!(
            command
                .as_std()
                .get_current_dir()
                .map(|path| path.to_string_lossy().into_owned()),
            Some(workspace_dir.to_string_lossy().into_owned())
        );
        assert_eq!(
            command_env_value(&command, "PROTON_NO_STEAMINPUT"),
            Some("1".to_string())
        );
        assert_eq!(
            command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
            Some(prefix_path.to_string_lossy().into_owned())
        );
        assert_eq!(
            command_env_value(&command, "WINEPREFIX"),
            Some(prefix_path.to_string_lossy().into_owned())
        );
        assert_eq!(command_env_value(&command, "GAMEID"), Some("0".to_string()));
    }

    #[test]
    fn proton_game_custom_env_overrides_duplicate_optimization_key() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let game_path = temp_dir.path().join("game.exe");
        let log_path = temp_dir.path().join("game.log");
        let steam_client_path = temp_dir.path().join("steam-client");
        let workspace_dir = prefix_path.join("workspace");

        fs::create_dir_all(&prefix_path).expect("prefix dir");
        fs::create_dir_all(&workspace_dir).expect("workspace dir");
        fs::create_dir_all(&steam_client_path).expect("steam client dir");
        write_executable_file(&proton_path);
        fs::write(&game_path, b"game").expect("game exe");

        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());
        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: game_path.to_string_lossy().into_owned(),
            trainer_path: String::new(),
            trainer_host_path: String::new(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig {
                app_id: String::new(),
                compatdata_path: String::new(),
                proton_path: String::new(),
                steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
            },
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: workspace_dir.to_string_lossy().into_owned(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest {
                enabled_option_ids: vec!["enable_dxvk_async".to_string()],
            },
            custom_env_vars: BTreeMap::from([("DXVK_ASYNC".to_string(), "0".to_string())]),
            launch_trainer_only: false,
            launch_game_only: true,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_game_command(&request, &log_path).expect("game command");
        assert_eq!(
            command_env_value(&command, "DXVK_ASYNC"),
            Some("0".to_string())
        );
        assert_eq!(command_env_value(&command, "GAMEID"), Some("0".to_string()));
    }

    #[test]
    fn native_game_command_applies_custom_env_vars() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let game_path = temp_dir.path().join("run.sh");
        write_executable_file(&game_path);
        let log_path = temp_dir.path().join("native.log");

        let request = LaunchRequest {
            method: crate::launch::METHOD_NATIVE.to_string(),
            game_path: game_path.to_string_lossy().into_owned(),
            trainer_path: String::new(),
            trainer_host_path: String::new(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig::default(),
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            custom_env_vars: BTreeMap::from([("NATIVE_TEST_VAR".to_string(), "hello".to_string())]),
            launch_trainer_only: false,
            launch_game_only: true,
            profile_name: None,
            ..Default::default()
        };

        let command = build_native_game_command(&request, &log_path).expect("native command");
        assert_eq!(
            command_env_value(&command, "NATIVE_TEST_VAR"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn proton_trainer_command_ignores_game_optimization_wrappers_and_env() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let wrapper_dir = temp_dir.path().join("wrappers");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_host_path = trainer_source_dir.join("sample.exe");
        let trainer_support_path = trainer_source_dir.join("sample.ini");
        let log_path = temp_dir.path().join("trainer.log");
        let workspace_dir = prefix_path.join("workspace");
        let wine_prefix_path = prefix_path.join("pfx");

        fs::create_dir_all(&wrapper_dir).expect("wrapper dir");
        fs::create_dir_all(wine_prefix_path.join("drive_c")).expect("wine prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer dir");
        fs::create_dir_all(&workspace_dir).expect("workspace dir");
        write_executable_file(&wrapper_dir.join("mangohud"));
        write_executable_file(&wrapper_dir.join("gamemoderun"));
        write_executable_file(&proton_path);
        fs::write(&trainer_host_path, b"trainer").expect("trainer exe");
        fs::write(&trainer_support_path, b"config").expect("trainer config");

        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(&wrapper_dir);
        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_host_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_host_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
            steam: crate::launch::SteamLaunchConfig {
                app_id: String::new(),
                compatdata_path: String::new(),
                proton_path: String::new(),
                steam_client_install_path: String::new(),
            },
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: workspace_dir.to_string_lossy().into_owned(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest {
                enabled_option_ids: vec![
                    "disable_steam_input".to_string(),
                    "show_mangohud_overlay".to_string(),
                    "use_gamemode".to_string(),
                ],
            },
            launch_trainer_only: true,
            launch_game_only: false,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");

        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            command.as_std().get_program().to_string_lossy(),
            proton_path.to_string_lossy()
        );
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "C:\\CrossHook\\StagedTrainers\\sample\\sample.exe".to_string(),
            ]
        );
        assert_eq!(
            command
                .as_std()
                .get_current_dir()
                .map(|path| path.to_string_lossy().into_owned()),
            Some(workspace_dir.to_string_lossy().into_owned())
        );
        assert_eq!(
            command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
            Some(prefix_path.to_string_lossy().into_owned())
        );
        assert_eq!(
            command_env_value(&command, "WINEPREFIX"),
            Some(wine_prefix_path.to_string_lossy().into_owned())
        );
        assert_eq!(command_env_value(&command, "GAMEID"), Some("0".to_string()));
        assert_eq!(command_env_value(&command, "PROTON_NO_STEAMINPUT"), None);
        assert_eq!(command_env_value(&command, "DXVK_ASYNC"), None);
        assert_eq!(command_env_value(&command, "MANGOHUD_CONFIGFILE"), None);
        assert!(wine_prefix_path
            .join("drive_c/CrossHook/StagedTrainers/sample/sample.ini")
            .exists());
    }

    #[test]
    fn helper_command_includes_expected_script_arguments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("script.sh");
        let log_path = temp_dir.path().join("log.txt");
        let request = steam_request();

        let command =
            build_helper_command(&request, &script_path, &log_path).expect("helper command");

        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                script_path.to_string_lossy().into_owned(),
                "--appid".to_string(),
                "12345".to_string(),
                "--compatdata".to_string(),
                "/tmp/compat".to_string(),
                "--proton".to_string(),
                "/tmp/proton".to_string(),
                "--steam-client".to_string(),
                "/tmp/steam".to_string(),
                "--game-exe-name".to_string(),
                "game.exe".to_string(),
                "--trainer-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--trainer-host-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--trainer-loading-mode".to_string(),
                "source_directory".to_string(),
                "--log-file".to_string(),
                log_path.to_string_lossy().into_owned(),
                "--game-startup-delay-seconds".to_string(),
                "30".to_string(),
                "--game-timeout-seconds".to_string(),
                "90".to_string(),
                "--trainer-timeout-seconds".to_string(),
                "10".to_string(),
                "--game-only".to_string(),
            ]
        );
    }

    #[test]
    fn helper_command_includes_working_directory_when_configured() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-helper.sh");
        let log_path = temp_dir.path().join("helper.log");
        let mut request = steam_request();
        request.runtime.working_directory = "/games/example".to_string();

        let command =
            build_helper_command(&request, &script_path, &log_path).expect("helper command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"--working-directory".to_string()));
        assert!(args.contains(&"/games/example".to_string()));
    }

    #[test]
    fn trainer_command_includes_steam_app_id_and_trainer_arguments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-trainer.sh");
        let log_path = temp_dir.path().join("trainer.log");
        let request = steam_request();

        let command =
            build_trainer_command(&request, &script_path, &log_path).expect("trainer command");

        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                script_path.to_string_lossy().into_owned(),
                "--compatdata".to_string(),
                "/tmp/compat".to_string(),
                "--proton".to_string(),
                "/tmp/proton".to_string(),
                "--steam-client".to_string(),
                "/tmp/steam".to_string(),
                "--steam-app-id".to_string(),
                "12345".to_string(),
                "--trainer-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--trainer-host-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--trainer-loading-mode".to_string(),
                "source_directory".to_string(),
                "--log-file".to_string(),
                log_path.to_string_lossy().into_owned(),
            ]
        );
    }

    #[test]
    fn trainer_command_ignores_launch_optimization_env() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-trainer.sh");
        let log_path = temp_dir.path().join("trainer.log");
        let mut request = steam_request();
        request.optimizations.enabled_option_ids = vec![
            "disable_steam_input".to_string(),
            "enable_dxvk_async".to_string(),
        ];

        let command =
            build_trainer_command(&request, &script_path, &log_path).expect("trainer command");

        assert_eq!(command_env_value(&command, "PROTON_NO_STEAMINPUT"), None);
        assert_eq!(command_env_value(&command, "DXVK_ASYNC"), None);
        assert_eq!(
            command_env_value(&command, "CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS"),
            None
        );
        assert_eq!(
            command_env_value(&command, "CROSSHOOK_TRAINER_CUSTOM_ENV_KEYS"),
            None
        );
        assert_eq!(
            command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
            Some("/tmp/compat".to_string())
        );
    }

    #[test]
    fn helper_command_includes_trainer_gamescope_when_configured() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-helper.sh");
        let log_path = temp_dir.path().join("helper.log");
        let mut request = steam_request();
        request.launch_game_only = false;
        request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
            enabled: true,
            allow_nested: true,
            fullscreen: true,
            ..Default::default()
        });

        let command =
            build_helper_command(&request, &script_path, &log_path).expect("helper command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"--gamescope-enabled".to_string()));
        assert!(args.contains(&"--gamescope-allow-nested".to_string()));
        assert!(args.contains(&"--gamescope-arg".to_string()));
        assert!(args.contains(&"-f".to_string()));
    }

    #[test]
    fn trainer_command_includes_working_directory_when_configured() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-trainer.sh");
        let log_path = temp_dir.path().join("trainer.log");
        let mut request = steam_request();
        request.runtime.working_directory = "/games/example".to_string();

        let command =
            build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"--working-directory".to_string()));
        assert!(args.contains(&"/games/example".to_string()));
    }

    #[test]
    fn flatpak_steam_trainer_command_reuses_direct_proton_builder_inputs() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let compatdata_path = temp_dir.path().join("compatdata");
        let proton_path = temp_dir.path().join("proton");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("sample.exe");
        let steam_client_path = temp_dir.path().join("steam-client");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(compatdata_path.join("pfx/drive_c")).expect("compatdata dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::create_dir_all(steam_client_path.join("steamapps"))
            .expect("steam client steamapps dir");
        fs::create_dir_all(steam_client_path.join("config")).expect("steam client config dir");
        write_executable_file(&proton_path);
        fs::write(&trainer_path, b"trainer").expect("trainer exe");

        let request = LaunchRequest {
            method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig {
                app_id: "12345".to_string(),
                compatdata_path: compatdata_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
            },
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: String::new(),
                proton_path: String::new(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            profile_name: None,
            ..Default::default()
        };

        let command = build_flatpak_steam_trainer_command(&request, &log_path)
            .expect("flatpak steam trainer command");

        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "run".to_string(),
                trainer_path.to_string_lossy().into_owned()
            ]
        );
        assert_eq!(
            command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
            Some(compatdata_path.to_string_lossy().into_owned())
        );
        assert_eq!(
            command_env_value(&command, "WINEPREFIX"),
            Some(compatdata_path.join("pfx").to_string_lossy().into_owned())
        );
        assert_eq!(
            command_env_value(&command, "STEAM_COMPAT_CLIENT_INSTALL_PATH"),
            Some(steam_client_path.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn resolve_launch_proton_path_with_mode_keeps_system_tool_without_local_override() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let steam_root = temp_dir.path().join("Steam");
        write_steam_client_root(&steam_root);

        let proton_path =
            "/usr/share/steam/compatibilitytools.d/crosshook-missing-system-tool/proton"
                .to_string();

        let resolved = resolve_launch_proton_path_with_mode(
            &proton_path,
            steam_root.to_string_lossy().as_ref(),
            true,
        );

        assert_eq!(resolved, proton_path);
    }

    #[test]
    fn resolve_launch_proton_path_with_mode_prefers_matching_local_tool_in_flatpak() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let steam_root = temp_dir.path().join("Steam");
        let local_tool = steam_root.join("compatibilitytools.d/Proton-CachyOS-SLR");
        write_steam_client_root(&steam_root);
        fs::create_dir_all(&local_tool).expect("local tool dir");
        fs::write(local_tool.join("proton"), b"proton").expect("local proton");

        let resolved = resolve_launch_proton_path_with_mode(
            "/usr/share/steam/compatibilitytools.d/Proton-CachyOS-SLR/proton",
            steam_root.to_string_lossy().as_ref(),
            true,
        );

        assert_eq!(resolved, local_tool.join("proton").to_string_lossy());
    }

    #[test]
    fn proton_trainer_command_stages_support_files_into_prefix() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("pfx");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("sample.exe");
        let trainer_config_path = trainer_source_dir.join("sample.ini");
        let proton_path = temp_dir.path().join("proton");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::write(&trainer_path, b"trainer").expect("trainer exe");
        fs::write(&trainer_config_path, b"config").expect("trainer config");
        write_executable_file(&proton_path);

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "C:\\CrossHook\\StagedTrainers\\sample\\sample.exe".to_string(),
            ]
        );
        assert!(prefix_path
            .join("drive_c/CrossHook/StagedTrainers/sample/sample.exe")
            .exists());
        assert!(prefix_path
            .join("drive_c/CrossHook/StagedTrainers/sample/sample.ini")
            .exists());
    }

    #[test]
    fn proton_trainer_command_uses_source_directory_without_staging() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("sample.exe");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::write(&trainer_path, b"trainer").expect("trainer exe");
        write_executable_file(&proton_path);

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "run".to_string(),
                trainer_path.to_string_lossy().into_owned()
            ]
        );
        assert_eq!(
            command
                .as_std()
                .get_current_dir()
                .map(|path| path.to_string_lossy().into_owned()),
            Some(trainer_source_dir.to_string_lossy().into_owned())
        );
        assert!(!prefix_path
            .join("drive_c/CrossHook/StagedTrainers/sample")
            .exists());
    }

    #[test]
    fn proton_trainer_command_prefers_enabled_trainer_gamescope() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("sample.exe");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::write(&trainer_path, b"trainer").expect("trainer exe");
        write_executable_file(&proton_path);

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            gamescope: crate::profile::GamescopeConfig::default(),
            trainer_gamescope: Some(crate::profile::GamescopeConfig {
                enabled: true,
                internal_width: Some(1280),
                internal_height: Some(720),
                ..Default::default()
            }),
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            command.as_std().get_program().to_string_lossy(),
            "gamescope"
        );
        assert!(args.contains(&"-w".to_string()));
        assert!(args.contains(&"1280".to_string()));
        assert!(args.contains(&"-h".to_string()));
        assert!(args.contains(&"720".to_string()));
        assert!(args.contains(&"--".to_string()));
        assert!(args.contains(&proton_path.to_string_lossy().into_owned()));
    }

    #[test]
    fn trainer_command_for_steam_inherits_enabled_game_gamescope_when_trainer_disabled() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-trainer.sh");
        let log_path = temp_dir.path().join("trainer.log");
        let mut request = steam_request();
        request.launch_trainer_only = true;
        request.launch_game_only = false;
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        };
        request.trainer_gamescope = Some(crate::profile::GamescopeConfig::default());

        let command =
            build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"--gamescope-enabled".to_string()));
        assert!(!args.contains(&"--gamescope-inherit-runtime".to_string()));
        assert!(
            !args.contains(&"-f".to_string()),
            "auto-generated trainer gamescope should clear fullscreen: {args:?}"
        );
    }

    #[test]
    fn trainer_command_for_steam_includes_allow_nested_when_trainer_gamescope_allows_nested() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("steam-launch-trainer.sh");
        let log_path = temp_dir.path().join("trainer.log");
        let mut request = steam_request();
        request.launch_trainer_only = true;
        request.launch_game_only = false;
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        };
        request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
            enabled: true,
            allow_nested: true,
            fullscreen: true,
            ..Default::default()
        });

        let command =
            build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"--gamescope-enabled".to_string()));
        assert!(args.contains(&"--gamescope-allow-nested".to_string()));
    }

    #[test]
    fn proton_game_command_sets_compat_data_path_for_standalone_prefixes() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("standalone-prefix");
        let proton_path = temp_dir.path().join("proton");
        let game_path = temp_dir.path().join("game.exe");
        let log_path = temp_dir.path().join("game.log");
        let steam_client_path = temp_dir.path().join("steam");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(steam_client_path.join("steamapps"))
            .expect("steam client steamapps dir");
        fs::create_dir_all(steam_client_path.join("config")).expect("steam client config dir");
        write_executable_file(&proton_path);
        fs::write(&game_path, b"game").expect("game exe");

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: game_path.to_string_lossy().into_owned(),
            trainer_path: String::new(),
            trainer_host_path: String::new(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig {
                app_id: String::new(),
                compatdata_path: String::new(),
                proton_path: String::new(),
                steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
            },
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: false,
            launch_game_only: true,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_game_command(&request, &log_path).expect("game command");
        let envs = command
            .as_std()
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|inner| inner.to_string_lossy().into_owned()),
                )
            })
            .collect::<Vec<_>>();

        assert!(envs.iter().any(|(key, value)| {
            key == "STEAM_COMPAT_DATA_PATH"
                && value.as_deref() == Some(prefix_path.to_string_lossy().as_ref())
        }));
        assert!(envs.iter().any(|(key, value)| {
            key == "WINEPREFIX" && value.as_deref() == Some(prefix_path.to_string_lossy().as_ref())
        }));
        assert!(envs.iter().any(|(key, value)| {
            key == "STEAM_COMPAT_CLIENT_INSTALL_PATH"
                && value.as_deref() == Some(steam_client_path.to_string_lossy().as_ref())
        }));
    }

    #[test]
    fn proton_trainer_command_uses_pfx_child_when_prefix_path_is_compatdata_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let compatdata_root = temp_dir.path().join("compatdata-root");
        let wine_prefix_path = compatdata_root.join("pfx");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("aurora.exe");
        let proton_path = temp_dir.path().join("proton");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(wine_prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::write(&trainer_path, b"trainer").expect("trainer exe");
        write_executable_file(&proton_path);

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: compatdata_root.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
        let envs = command
            .as_std()
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|inner| inner.to_string_lossy().into_owned()),
                )
            })
            .collect::<Vec<_>>();

        assert!(wine_prefix_path
            .join("drive_c/CrossHook/StagedTrainers/aurora/aurora.exe")
            .exists());
        assert!(envs.iter().any(|(key, value)| {
            key == "WINEPREFIX"
                && value.as_deref() == Some(wine_prefix_path.to_string_lossy().as_ref())
        }));
        assert!(envs.iter().any(|(key, value)| {
            key == "STEAM_COMPAT_DATA_PATH"
                && value.as_deref() == Some(compatdata_root.to_string_lossy().as_ref())
        }));
    }

    #[test]
    #[cfg(unix)]
    fn copy_dir_all_skips_symlinks() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let destination_dir = temp_dir.path().join("destination");

        fs::create_dir_all(&source_dir).expect("source dir");

        // Create a regular file that should be copied
        fs::write(source_dir.join("real.dll"), b"content").expect("real file");

        // Create a symlink that should be skipped
        let external_target = temp_dir.path().join("external_target.dll");
        fs::write(&external_target, b"external").expect("external file");
        symlink(&external_target, source_dir.join("link.dll")).expect("symlink");

        copy_dir_all(&source_dir, &destination_dir).expect("copy_dir_all");

        assert!(
            destination_dir.join("real.dll").exists(),
            "regular file should be copied"
        );
        assert!(
            !destination_dir.join("link.dll").exists(),
            "symlink should be skipped"
        );
    }

    #[test]
    fn proton_trainer_command_prepends_unshare_net_when_isolation_enabled() {
        // Only meaningful when unshare --net is available. CI containers and
        // hardened kernels may not support this.
        if !crate::launch::runtime_helpers::is_unshare_net_available() {
            eprintln!("SKIP: unshare --net not available on this system");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("sample.exe");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::write(&trainer_path, b"trainer").expect("trainer exe");
        write_executable_file(&proton_path);

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            network_isolation: true,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
        assert_eq!(
            command.as_std().get_program().to_string_lossy(),
            "unshare",
            "first program should be unshare when network_isolation is enabled"
        );
        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "--net", "first arg should be --net");
    }

    #[test]
    fn proton_trainer_command_skips_unshare_when_isolation_disabled() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let trainer_source_dir = temp_dir.path().join("trainer");
        let trainer_path = trainer_source_dir.join("sample.exe");
        let log_path = temp_dir.path().join("trainer.log");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
        fs::write(&trainer_path, b"trainer").expect("trainer exe");
        write_executable_file(&proton_path);

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: temp_dir
                .path()
                .join("game.exe")
                .to_string_lossy()
                .into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            trainer_host_path: trainer_path.to_string_lossy().into_owned(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: true,
            launch_game_only: false,
            network_isolation: false,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
        let program = command
            .as_std()
            .get_program()
            .to_string_lossy()
            .into_owned();
        assert_ne!(
            program, "unshare",
            "should NOT use unshare when network_isolation is false"
        );
    }

    #[test]
    fn proton_game_command_does_not_include_unshare_net() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        let game_path = temp_dir.path().join("game.exe");
        let log_path = temp_dir.path().join("game.log");
        let steam_client_path = temp_dir.path().join("steam-client");

        fs::create_dir_all(&prefix_path).expect("prefix dir");
        fs::create_dir_all(&steam_client_path).expect("steam client dir");
        write_executable_file(&proton_path);
        fs::write(&game_path, b"game").expect("game exe");

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: game_path.to_string_lossy().into_owned(),
            trainer_path: String::new(),
            trainer_host_path: String::new(),
            trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            steam: crate::launch::SteamLaunchConfig {
                app_id: String::new(),
                compatdata_path: String::new(),
                proton_path: String::new(),
                steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
            },
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
            launch_trainer_only: false,
            launch_game_only: true,
            network_isolation: true,
            profile_name: None,
            ..Default::default()
        };

        let command = build_proton_game_command(&request, &log_path).expect("game command");
        let program = command
            .as_std()
            .get_program()
            .to_string_lossy()
            .into_owned();
        assert_ne!(
            program, "unshare",
            "game command must NEVER use unshare --net"
        );
        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(
            !args.contains(&"--net".to_string()),
            "game command args must not contain --net"
        );
    }
}
