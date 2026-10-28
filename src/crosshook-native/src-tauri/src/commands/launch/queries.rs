use std::collections::BTreeMap;

use crosshook_app::services::launch::LaunchService;
use crosshook_app::services::HostToolsService;
use crosshook_core::launch::{
    LaunchPlatformCapabilities, LaunchPreview, LaunchRequest, LaunchValidationIssue,
};
use crosshook_core::metadata::LaunchHistoryEntry;
use crosshook_core::profile::GamescopeConfig;
use tauri::State;

#[tauri::command]
pub fn validate_launch(
    request: LaunchRequest,
    service: State<'_, LaunchService>,
) -> Result<(), LaunchValidationIssue> {
    service.validate(&request)
}

#[tauri::command]
pub async fn preview_launch(
    request: LaunchRequest,
    service: State<'_, LaunchService>,
) -> Result<LaunchPreview, String> {
    service
        .preview(request)
        .await
        .map_err(|error| error.to_string())
}

/// Builds a Steam per-game “Launch Options” line from the same optimization IDs as `proton_run`,
/// plus profile custom env vars (custom wins on duplicate keys in the prefix).
///
/// When `gamescope` is provided and enabled, the gamescope compositor is inserted as a wrapper
/// (e.g. `gamescope -w 2560 -h 1440 -f -- %command%`). Resolved command-argument tokens are
/// appended after `%command%` when `enabled_argument_ids` or `custom_command_args` are non-empty.
#[tauri::command]
pub fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
    custom_env_vars: BTreeMap<String, String>,
    gamescope: Option<GamescopeConfig>,
    enabled_argument_ids: Vec<String>,
    custom_command_args: Vec<String>,
    service: State<'_, LaunchService>,
) -> Result<String, String> {
    service
        .steam_launch_options_command(
            &enabled_option_ids,
            &custom_env_vars,
            gamescope.as_ref(),
            &enabled_argument_ids,
            &custom_command_args,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn check_gamescope_session(service: State<'_, HostToolsService>) -> Result<bool, String> {
    Ok(service.gamescope_session())
}

/// Returns Flatpak sandbox and host capability flags for launch UI (not persisted).
#[tauri::command]
pub async fn launch_platform_status(
    service: State<'_, HostToolsService>,
) -> Result<LaunchPlatformCapabilities, String> {
    service
        .platform_status()
        .await
        .map_err(|error| error.message().to_string())
}

#[tauri::command]
pub fn check_game_running(exe_name: String) -> bool {
    let name = exe_name.trim();
    if name.is_empty() {
        return false;
    }
    crosshook_core::launch::is_process_running(name)
}

#[tauri::command]
pub fn list_running_profiles(service: State<'_, LaunchService>) -> Vec<String> {
    service.running_profiles()
}

/// Recent launch rows for a profile (newest first), from `launch_operations` — no `diagnostic_json`.
#[tauri::command]
pub async fn list_launch_history_for_profile(
    profile_name: String,
    limit: Option<u32>,
    service: State<'_, LaunchService>,
) -> Result<Vec<LaunchHistoryEntry>, String> {
    service
        .launch_history(profile_name, limit)
        .await
        .map_err(|error| error.to_string())
}
