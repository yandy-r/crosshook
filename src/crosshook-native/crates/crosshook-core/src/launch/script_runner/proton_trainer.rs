use std::collections::BTreeMap;
use std::path::Path;

use tokio::process::Command;

use super::common::{prepare_gamescope_launch, should_skip_gamescope};
use super::proton_resolution::resolve_launch_proton_path;
use super::trainer_staging::stage_trainer_into_prefix;
use super::umu::{
    proton_path_dirname, resolved_umu_game_id_for_env, should_use_umu, warn_on_umu_fallback,
};
use crate::launch::runtime_helpers::{
    build_direct_proton_command_with_wrappers_in_directory,
    build_proton_command_with_gamescope_in_directory, collect_pressure_vessel_paths,
    host_environment_map, is_unshare_net_available, merge_runtime_proton_into_map,
    resolve_effective_working_directory,
};
use crate::launch::{LaunchRequest, METHOD_PROTON_RUN};
use crate::platform::normalize_flatpak_host_path;
use crate::profile::TrainerLoadingMode;

pub fn build_flatpak_steam_trainer_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let mut direct_request = request.clone();
    direct_request.method = METHOD_PROTON_RUN.to_string();
    direct_request.runtime.prefix_path =
        normalize_flatpak_host_path(&request.steam.compatdata_path);
    direct_request.runtime.proton_path = request.steam.proton_path.clone();

    build_proton_trainer_command_with_umu_override(
        &direct_request,
        log_path,
        /*force_no_umu=*/ true,
    )
}

pub fn build_proton_trainer_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    build_proton_trainer_command_with_umu_override(request, log_path, /*force_no_umu=*/ false)
}

pub(super) fn build_proton_trainer_command_with_umu_override(
    request: &LaunchRequest,
    _log_path: &Path,
    force_no_umu: bool,
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

    let effective_wrappers = if request.network_isolation && is_unshare_net_available() {
        vec!["unshare".to_string(), "--net".to_string()]
    } else {
        Vec::new()
    };

    let trainer_gamescope = request.resolved_trainer_gamescope();
    let gamescope_active = trainer_gamescope.enabled && !should_skip_gamescope(&trainer_gamescope);

    let (use_umu, umu_run_path) = should_use_umu(request, force_no_umu);
    if !use_umu && !force_no_umu {
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
    if use_umu {
        env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
        env.insert("PROTON_VERB".to_string(), "runinprefix".to_string());
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
    let program_path = if use_umu {
        umu_run_path
            .as_deref()
            .expect("use_umu implies umu_run_path")
            .to_string()
    } else {
        resolved_proton_path.clone()
    };
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
        use_umu,
        umu_run_path = umu_run_path.as_deref().unwrap_or(""),
        "building proton trainer launch"
    );

    let mut command = if gamescope_active {
        let (gamescope_args, filtered_wrappers) =
            prepare_gamescope_launch(&trainer_gamescope, &effective_wrappers);
        build_proton_command_with_gamescope_in_directory(
            program_path.as_str(),
            &filtered_wrappers,
            &gamescope_args,
            &env,
            effective_working_directory.as_deref(),
            &BTreeMap::new(),
            use_umu,
        )
    } else {
        build_direct_proton_command_with_wrappers_in_directory(
            program_path.as_str(),
            &effective_wrappers,
            &env,
            effective_working_directory.as_deref(),
            &BTreeMap::new(),
            use_umu,
        )
    };
    command.arg(trainer_launch_path);
    Ok(command)
}
