use std::collections::BTreeMap;

use crosshook_core::launch::{
    build_launch_preview,
    build_steam_launch_options_command as build_steam_launch_options_command_core, validate,
    LaunchPlatformCapabilities, LaunchPreview, LaunchRequest, LaunchValidationIssue,
};
use crosshook_core::profile::GamescopeConfig;

#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue> {
    validate(&request).map_err(|error| error.issue())
}

#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}

/// Builds a Steam per-game “Launch Options” line from the same optimization IDs as `proton_run`,
/// plus profile custom env vars (custom wins on duplicate keys in the prefix).
///
/// When `gamescope` is provided and enabled, the gamescope compositor is inserted as a wrapper
/// (e.g. `gamescope -w 2560 -h 1440 -f -- %command%`).
#[tauri::command]
pub fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
    custom_env_vars: BTreeMap<String, String>,
    gamescope: Option<GamescopeConfig>,
) -> Result<String, String> {
    build_steam_launch_options_command_core(
        &enabled_option_ids,
        &custom_env_vars,
        gamescope.as_ref(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn check_gamescope_session() -> bool {
    crosshook_core::launch::is_inside_gamescope_session()
}

/// Returns Flatpak sandbox and host capability flags for launch UI (not persisted).
#[tauri::command]
pub fn launch_platform_status() -> LaunchPlatformCapabilities {
    crosshook_core::launch::launch_platform_capabilities()
}

#[tauri::command]
pub fn check_game_running(exe_name: String) -> bool {
    let name = exe_name.trim();
    if name.is_empty() {
        return false;
    }
    crosshook_core::launch::is_process_running(name)
}
