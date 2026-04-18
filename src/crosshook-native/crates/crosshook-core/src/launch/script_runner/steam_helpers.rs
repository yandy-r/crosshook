use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use tokio::process::Command;

use super::common::{
    collect_trainer_builtin_env_keys, insert_sorted_env_key_list,
    merge_mangohud_config_env_into_map, should_skip_gamescope, validation_error_to_io_error,
};
use super::proton_resolution::resolve_launch_proton_path;
use crate::launch::runtime_helpers::{
    build_gamescope_args, host_environment_map, is_unshare_net_available,
    merge_optimization_and_custom_into_map,
};
use crate::launch::{resolve_launch_directives_for_method, LaunchRequest, METHOD_PROTON_RUN};
use crate::platform::{self, host_command_with_env, normalize_flatpak_host_path};
use crate::profile::GamescopeConfig;

const BASH_EXECUTABLE: &str = "/bin/bash";
const DEFAULT_GAME_STARTUP_DELAY_SECONDS: &str = "30";
const DEFAULT_GAME_TIMEOUT_SECONDS: &str = "90";
const DEFAULT_TRAINER_TIMEOUT_SECONDS: &str = "10";

fn build_flatpak_unshare_bash_command(
    script_path: &Path,
    env: &BTreeMap<String, String>,
    custom_env: &BTreeMap<String, String>,
) -> std::io::Result<Command> {
    let mut base = env.clone();
    for key in custom_env.keys() {
        base.remove(key);
    }
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
        && is_unshare_net_available()
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

    let mut command = if request.network_isolation && is_unshare_net_available() {
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
