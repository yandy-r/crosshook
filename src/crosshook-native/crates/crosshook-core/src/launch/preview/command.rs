use std::path::Path;

use super::super::optimizations::build_steam_launch_options_command;
use super::super::request::LaunchRequest;
use super::super::runtime_helpers::build_gamescope_args;
use super::super::script_runner::{force_no_umu_for_launch_request, should_use_umu};
use super::types::ResolvedLaunchMethod;
use crate::profile::{GamescopeConfig, TrainerLoadingMode};

/// Builds a human-readable command string showing the effective launch command.
pub(super) fn build_effective_command_string(
    request: &LaunchRequest,
    method: ResolvedLaunchMethod,
    effective_wrappers: &[String],
    gamescope_config: &GamescopeConfig,
    gamescope_active: bool,
) -> Result<String, String> {
    match method {
        ResolvedLaunchMethod::ProtonRun => {
            let mut parts: Vec<String> = Vec::new();
            let force_no_umu = force_no_umu_for_launch_request(request);
            let (use_umu, umu_run_path) = should_use_umu(request, force_no_umu);

            if gamescope_active {
                // Apply MangoHud → mangoapp swap: if wrappers contain "mangohud", remove it and
                // add "--mangoapp" to the gamescope args instead.
                let mut gamescope_args = build_gamescope_args(gamescope_config);
                let wrappers_without_mangohud: Vec<String> = effective_wrappers
                    .iter()
                    .filter(|w| *w != "mangohud")
                    .cloned()
                    .collect();
                let had_mangohud = wrappers_without_mangohud.len() != effective_wrappers.len();
                if had_mangohud {
                    gamescope_args.push("--mangoapp".to_string());
                }

                parts.push("gamescope".to_string());
                parts.extend(gamescope_args);
                parts.push("--".to_string());
                for wrapper in &wrappers_without_mangohud {
                    parts.push(wrapper.clone());
                }
            } else {
                for wrapper in effective_wrappers {
                    parts.push(wrapper.to_string());
                }
            }

            if use_umu {
                parts.push(umu_run_path.unwrap_or_default());
            } else {
                parts.push(request.runtime.proton_path.trim().to_string());
                parts.push("run".to_string());
            }
            if request.launch_trainer_only {
                parts.push(resolve_trainer_launch_path_for_preview(request));
            } else {
                parts.push(request.game_path.trim().to_string());
            }
            Ok(parts.join(" "))
        }
        ResolvedLaunchMethod::SteamApplaunch => {
            let gs = if gamescope_config.enabled {
                Some(gamescope_config)
            } else {
                None
            };
            build_steam_launch_options_command(
                &request.optimizations.enabled_option_ids,
                &request.custom_env_vars,
                gs,
            )
            .map_err(|error| error.to_string())
        }
        ResolvedLaunchMethod::Native => Ok(request.game_path.trim().to_string()),
    }
}

pub(super) fn resolve_trainer_launch_path_for_preview(request: &LaunchRequest) -> String {
    match request.trainer_loading_mode {
        TrainerLoadingMode::SourceDirectory => request.trainer_host_path.trim().to_string(),
        TrainerLoadingMode::CopyToPrefix => {
            let path = Path::new(request.trainer_host_path.trim());
            let file_stem = path
                .file_stem()
                .map(|segment| segment.to_string_lossy().into_owned())
                .unwrap_or_default();
            let file_name = path
                .file_name()
                .map(|segment| segment.to_string_lossy().into_owned())
                .unwrap_or_default();

            if file_stem.is_empty() || file_name.is_empty() {
                request.trainer_host_path.trim().to_string()
            } else {
                format!("C:\\CrossHook\\StagedTrainers\\{file_stem}\\{file_name}")
            }
        }
    }
}
