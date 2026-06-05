use std::collections::BTreeMap;
use std::path::Path;

use tokio::process::Command;

use super::common::{
    gamescope_pid_capture_path, merge_mangohud_config_env_into_map, prepare_gamescope_launch,
    should_skip_gamescope, validation_error_to_io_error,
};
use super::proton_resolution::resolve_launch_proton_path;
use super::umu::{
    proton_path_dirname, resolved_umu_game_id_for_env, should_use_umu, warn_on_umu_fallback,
};
use crate::launch::runtime_helpers::{
    build_direct_proton_command_with_wrappers_in_directory,
    build_proton_command_with_gamescope_in_directory,
    build_proton_command_with_gamescope_pid_capture_in_directory, collect_pressure_vessel_paths,
    host_environment_map, merge_optimization_and_custom_into_map, merge_runtime_proton_into_map,
    resolve_effective_working_directory,
};
use crate::launch::{resolve_launch_directives, LaunchRequest};
use crate::platform::{self, normalize_flatpak_host_path};

pub fn build_proton_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let directives = resolve_launch_directives(request).map_err(validation_error_to_io_error)?;
    let gamescope_active = request.gamescope.enabled && !should_skip_gamescope(&request.gamescope);
    let wrappers_had_mangohud = directives.wrappers.iter().any(|w| w.trim() == "mangohud");

    let (use_umu, umu_run_path) = should_use_umu(request, false);
    if !use_umu {
        warn_on_umu_fallback(request);
    }

    let resolved_proton_path = resolve_launch_proton_path(
        request.runtime.proton_path.trim(),
        request.steam.steam_client_install_path.trim(),
    );

    let mut env = host_environment_map();
    merge_runtime_proton_into_map(
        &mut env,
        request.runtime.prefix_path.trim(),
        request.steam.steam_client_install_path.trim(),
    );
    merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
    if use_umu {
        env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
        env.insert("PROTON_VERB".to_string(), "waitforexitandrun".to_string());
        env.insert(
            "PROTONPATH".to_string(),
            proton_path_dirname(resolved_proton_path.trim()),
        );
    }
    let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":");
    env.insert(
        "STEAM_COMPAT_LIBRARY_PATHS".to_string(),
        pressure_vessel_paths.clone(),
    );
    env.insert(
        "PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(),
        pressure_vessel_paths,
    );
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
    let resolved_steam_client_install_path = env
        .get("STEAM_COMPAT_CLIENT_INSTALL_PATH")
        .map(String::as_str)
        .unwrap_or("");

    let program_path = if use_umu {
        umu_run_path
            .as_deref()
            .expect("use_umu implies umu_run_path")
            .to_string()
    } else {
        resolved_proton_path.clone()
    };

    tracing::debug!(
        configured_proton_path = request.runtime.proton_path.trim(),
        resolved_proton_path = resolved_proton_path.trim(),
        steam_client_install_path = resolved_steam_client_install_path,
        target_path = normalized_game_path.trim(),
        working_directory = effective_working_directory.as_deref().unwrap_or(""),
        gamescope_active,
        wrapper_count = directives.wrappers.len(),
        use_umu,
        umu_run_path = umu_run_path.as_deref().unwrap_or(""),
        "building proton game launch"
    );

    let mut command = if gamescope_active {
        let (gamescope_args, filtered_wrappers) =
            prepare_gamescope_launch(&request.gamescope, &directives.wrappers);
        if platform::is_flatpak() {
            let pid_capture_path = gamescope_pid_capture_path(log_path);
            build_proton_command_with_gamescope_pid_capture_in_directory(
                program_path.as_str(),
                &filtered_wrappers,
                &gamescope_args,
                &env,
                effective_working_directory.as_deref(),
                &request.custom_env_vars,
                Some(&pid_capture_path),
                use_umu,
            )
        } else {
            build_proton_command_with_gamescope_in_directory(
                program_path.as_str(),
                &filtered_wrappers,
                &gamescope_args,
                &env,
                effective_working_directory.as_deref(),
                &request.custom_env_vars,
                use_umu,
            )
        }
    } else {
        build_direct_proton_command_with_wrappers_in_directory(
            program_path.as_str(),
            &directives.wrappers,
            &env,
            effective_working_directory.as_deref(),
            &request.custom_env_vars,
            use_umu,
        )
    };
    command.arg(normalized_game_path.trim());
    Ok(command)
}
