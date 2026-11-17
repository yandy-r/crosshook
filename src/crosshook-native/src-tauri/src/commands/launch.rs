//! Launch Tauri commands: thin delegates to `crosshook_app::LaunchService`.
//!
//! The `launch-*` events are emitted by `crate::event_bridge`; the result and
//! payload DTOs live in `crosshook_app::ipc::launch`.

use std::collections::BTreeMap;

use crosshook_app::ipc::launch::LaunchResult;
use crosshook_app::ipc::sessions::RunningSessionsDto;
use crosshook_app::{LaunchSession, LaunchStartError};
use crosshook_core::launch::{LaunchPreview, LaunchRequest, LaunchValidationIssue};
use crosshook_core::metadata::LaunchHistoryEntry;
use crosshook_core::profile::GamescopeConfig;
use tauri::AppHandle;

use super::{AppState, IpcResult};
use crate::error_map::ipc;
use crate::event_bridge::{relay_launch, replay_launch_start_error};

/// Hands a started session to the relay and builds the IPC result; a failed
/// start replays the events published before the failure.
fn finish(
    handle: &AppHandle,
    started: Result<LaunchSession, LaunchStartError>,
    message: &str,
) -> IpcResult<LaunchResult> {
    match started {
        Ok(session) => {
            let (result, events) = LaunchResult::from_session(session, message);
            relay_launch(handle.clone(), events);
            Ok(result)
        }
        Err(error) => {
            let message = error.to_string();
            replay_launch_start_error(handle, error.events);
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn launch_game(
    handle: AppHandle,
    request: LaunchRequest,
    app: AppState<'_>,
) -> IpcResult<LaunchResult> {
    finish(
        &handle,
        app.services().launch.start_game(request).await,
        "Game launch started.",
    )
}

#[tauri::command]
pub async fn launch_trainer(
    handle: AppHandle,
    request: LaunchRequest,
    app: AppState<'_>,
) -> IpcResult<LaunchResult> {
    finish(
        &handle,
        app.services().launch.start_trainer(request).await,
        "Trainer launch started.",
    )
}

#[tauri::command]
pub async fn launch_reset_sessions(profile_name: String, app: AppState<'_>) -> IpcResult<usize> {
    Ok(app.services().launch.reset_sessions(&profile_name))
}

#[tauri::command]
pub async fn validate_launch(
    request: LaunchRequest,
    app: AppState<'_>,
) -> Result<(), LaunchValidationIssue> {
    app.services().launch.validate(&request)
}

#[tauri::command]
pub async fn preview_launch(request: LaunchRequest, app: AppState<'_>) -> IpcResult<LaunchPreview> {
    ipc(app.services().launch.preview(request).await)
}

/// Builds a Steam per-game “Launch Options” line from the same optimization IDs as `proton_run`,
/// plus profile custom env vars (custom wins on duplicate keys in the prefix).
///
/// When `gamescope` is provided and enabled, the gamescope compositor is inserted as a wrapper
/// (e.g. `gamescope -w 2560 -h 1440 -f -- %command%`). Resolved command-argument tokens are
/// appended after `%command%` when `enabled_argument_ids` or `custom_command_args` are non-empty.
#[tauri::command]
pub async fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
    custom_env_vars: BTreeMap<String, String>,
    gamescope: Option<GamescopeConfig>,
    enabled_argument_ids: Vec<String>,
    custom_command_args: Vec<String>,
    app: AppState<'_>,
) -> IpcResult<String> {
    ipc(app.services().launch.steam_launch_options_command(
        &enabled_option_ids,
        &custom_env_vars,
        gamescope.as_ref(),
        &enabled_argument_ids,
        &custom_command_args,
    ))
}

#[tauri::command]
pub async fn check_gamescope_session(app: AppState<'_>) -> IpcResult<bool> {
    Ok(app.services().host_tools.gamescope_session())
}

#[tauri::command]
pub async fn check_game_running(exe_name: String, app: AppState<'_>) -> IpcResult<bool> {
    Ok(app.services().launch.is_game_running(exe_name).await)
}

/// Every registered launch session plus the registry `seq` it reflects. The
/// UI subscribes to `launch-session-changed` first, then applies only events
/// with a greater `seq`.
#[tauri::command]
pub async fn list_running_sessions(app: AppState<'_>) -> IpcResult<RunningSessionsDto> {
    Ok(app.services().launch.running_sessions_snapshot().into())
}

/// Recent launch rows for a profile (newest first), from `launch_operations` — no `diagnostic_json`.
#[tauri::command]
pub async fn list_launch_history_for_profile(
    profile_name: String,
    limit: Option<u32>,
    app: AppState<'_>,
) -> IpcResult<Vec<LaunchHistoryEntry>> {
    ipc(app
        .services()
        .launch
        .launch_history(profile_name, limit)
        .await)
}
