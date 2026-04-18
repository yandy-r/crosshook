use std::path::Path;

use super::types::{PreviewTrainerInfo, ProtonSetup};
use crate::launch::request::{
    LaunchRequest, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crate::launch::runtime_helpers::{resolve_proton_paths, resolve_steam_client_install_path};
use crate::launch::script_runner::{force_no_umu_for_launch_request, should_use_umu};
use crate::profile::TrainerLoadingMode;

/// Builds Proton setup details. Returns `None` for native method.
pub(super) fn build_proton_setup(request: &LaunchRequest, method: &str) -> Option<ProtonSetup> {
    match method {
        METHOD_PROTON_RUN => {
            let resolved_paths =
                resolve_proton_paths(Path::new(request.runtime.prefix_path.trim()));

            let steam_client =
                resolve_steam_client_install_path(request.steam.steam_client_install_path.trim())
                    .unwrap_or_default();

            let force_no_umu = force_no_umu_for_launch_request(request);
            let (use_umu, umu_run_path) = should_use_umu(request, force_no_umu);
            let umu_run_path_field = if use_umu { umu_run_path } else { None };

            Some(ProtonSetup {
                wine_prefix_path: resolved_paths
                    .wine_prefix_path
                    .to_string_lossy()
                    .into_owned(),
                compat_data_path: resolved_paths
                    .compat_data_path
                    .to_string_lossy()
                    .into_owned(),
                steam_client_install_path: steam_client,
                proton_executable: request.runtime.proton_path.trim().to_string(),
                umu_run_path: umu_run_path_field,
            })
        }
        METHOD_STEAM_APPLAUNCH => {
            // Steam opt-out: never reflect umu in the preview setup.
            let compatdata = request.steam.compatdata_path.trim();

            Some(ProtonSetup {
                wine_prefix_path: Path::new(compatdata)
                    .join("pfx")
                    .to_string_lossy()
                    .into_owned(),
                compat_data_path: compatdata.to_string(),
                steam_client_install_path: request
                    .steam
                    .steam_client_install_path
                    .trim()
                    .to_string(),
                proton_executable: request.steam.proton_path.trim().to_string(),
                umu_run_path: None,
            })
        }
        _ => None,
    }
}

/// Builds trainer info. Returns `None` if trainer path is empty or native method.
///
/// For `copy_to_prefix` mode, computes the staged path via string manipulation
/// without calling `stage_trainer_into_prefix()` (which has side effects).
pub(super) fn build_trainer_info(
    request: &LaunchRequest,
    method: &str,
) -> Option<PreviewTrainerInfo> {
    if method == METHOD_NATIVE {
        return None;
    }

    let trainer_path = request.trainer_path.trim();
    if trainer_path.is_empty() {
        return None;
    }

    let host_path = request.trainer_host_path.trim().to_string();
    let loading_mode = request.trainer_loading_mode;

    let staged_path = if request.trainer_loading_mode == TrainerLoadingMode::CopyToPrefix {
        let path = Path::new(request.trainer_host_path.trim());
        let file_stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !file_stem.is_empty() && !file_name.is_empty() {
            Some(format!(
                "C:\\CrossHook\\StagedTrainers\\{file_stem}\\{file_name}"
            ))
        } else {
            None
        }
    } else {
        None
    };

    Some(PreviewTrainerInfo {
        path: trainer_path.to_string(),
        host_path,
        loading_mode,
        staged_path,
    })
}

/// Resolves the working directory shown in preview output.
///
/// `steam_applaunch` intentionally reports no working directory because
/// the launch path does not apply one at runtime.
pub(super) fn resolve_working_directory(request: &LaunchRequest, method: &str) -> String {
    if method == METHOD_STEAM_APPLAUNCH {
        return String::new();
    }

    let configured = request.runtime.working_directory.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }

    let game_path = Path::new(request.game_path.trim());
    game_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
