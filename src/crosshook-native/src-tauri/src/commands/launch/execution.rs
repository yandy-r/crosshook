use crosshook_app::services::launch::LaunchService;
use crosshook_app::LaunchSession;
use crosshook_core::launch::LaunchRequest;
use tauri::{AppHandle, State};

use super::relay;
use super::shared::LaunchResult;

/// Hands a started session to the relay and builds the IPC result.
fn started(app: AppHandle, session: LaunchSession, message: &str) -> LaunchResult {
    let LaunchSession {
        log_path,
        warnings,
        events,
        ..
    } = session;
    relay::spawn(app, events);
    LaunchResult {
        succeeded: true,
        message: message.to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
        warnings,
    }
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    request: LaunchRequest,
    service: State<'_, LaunchService>,
) -> Result<LaunchResult, String> {
    match service.start_game(request).await {
        Ok(session) => Ok(started(app, session, "Game launch started.")),
        Err(error) => {
            let message = error.to_string();
            relay::replay_start_error(&app, error.events);
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn launch_trainer(
    app: AppHandle,
    request: LaunchRequest,
    service: State<'_, LaunchService>,
) -> Result<LaunchResult, String> {
    match service.start_trainer(request).await {
        Ok(session) => Ok(started(app, session, "Trainer launch started.")),
        Err(error) => {
            let message = error.to_string();
            relay::replay_start_error(&app, error.events);
            Err(message)
        }
    }
}

/// User-initiated launch reset: cancels every active session registered for
/// the profile and returns how many were signalled.
#[tauri::command]
pub fn launch_reset_sessions(profile_name: String, service: State<'_, LaunchService>) -> usize {
    service.reset_sessions(&profile_name)
}
