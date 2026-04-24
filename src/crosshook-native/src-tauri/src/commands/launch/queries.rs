use std::{collections::BTreeMap, sync::Arc};

use crosshook_core::launch::{
    build_launch_preview,
    build_steam_launch_options_command as build_steam_launch_options_command_core, validate,
    LaunchPlatformCapabilities, LaunchPreview, LaunchRequest, LaunchSessionRegistry,
    LaunchValidationIssue, SessionKind,
};
use crosshook_core::metadata::{LaunchHistoryEntry, MetadataStore, MAX_HISTORY_LIST_LIMIT};
use crosshook_core::profile::GamescopeConfig;
use tauri::State;

fn map_error(e: impl ToString) -> String {
    e.to_string()
}

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

#[tauri::command]
pub fn list_running_profiles(
    session_registry: State<'_, Arc<LaunchSessionRegistry>>,
) -> Vec<String> {
    session_registry.active_profile_keys(Some(SessionKind::Game))
}

/// Recent launch rows for a profile (newest first), from `launch_operations` — no `diagnostic_json`.
#[tauri::command]
pub fn list_launch_history_for_profile(
    profile_name: String,
    limit: Option<u32>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<LaunchHistoryEntry>, String> {
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        return Err("profile_name is required".to_string());
    }
    if !metadata_store.is_available() {
        return Ok(Vec::new());
    }
    let cap = limit.unwrap_or(20).min(MAX_HISTORY_LIST_LIMIT as u32) as usize;
    metadata_store
        .query_launch_history_for_profile(profile_name, cap)
        .map_err(map_error)
}
