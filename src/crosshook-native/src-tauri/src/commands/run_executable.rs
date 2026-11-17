//! Thin IPC adapters over the run-executable service.
//!
//! The service owns validation, the supervised Proton child, cancel/stop and
//! throwaway-prefix cleanup. Errors use the frozen [`RunCommandError`]
//! envelope from `crosshook_app::ipc`; the run's events are relayed by
//! `crate::event_bridge`.

use crosshook_app::services::{JobOwner, RunExecutableResponse};
use crosshook_core::run_executable::RunExecutableRequest;
use tauri::AppHandle;

use super::AppState;
use crate::error_map::RunCommandError;
use crate::event_bridge::relay_run_executable;

// Only pass the label of the injected invoking webview, never a JS argument.
#[tauri::command]
pub async fn validate_run_executable_request(
    request: RunExecutableRequest,
    app: AppState<'_>,
) -> Result<(), RunCommandError> {
    app.services()
        .run_executable
        .validate(request)
        .await
        .map_err(RunCommandError::from)
}

/// Starts a supervised run for the invoking webview and streams its output
/// on `run-executable-log`, then `run-executable-complete`, both tagged with
/// the returned `run_id`.
#[tauri::command]
pub async fn run_executable(
    handle: AppHandle,
    webview: tauri::Webview,
    request: RunExecutableRequest,
    app: AppState<'_>,
) -> Result<RunExecutableResponse, RunCommandError> {
    let (response, run) = app
        .services()
        .run_executable
        .start(JobOwner::new(webview.label()), request)
        .await
        .map_err(RunCommandError::from)?;
    relay_run_executable(handle, run);
    Ok(response)
}

/// Polite cancellation of the invoking webview's run: `SIGTERM` to its
/// process group, `SIGKILL` after the grace period. No active run (or a run
/// owned by another webview) is a no-op.
#[tauri::command]
pub async fn cancel_run_executable(
    webview: tauri::Webview,
    app: AppState<'_>,
) -> Result<(), RunCommandError> {
    app.services()
        .run_executable
        .cancel(&JobOwner::new(webview.label()));
    Ok(())
}

/// Immediate `SIGKILL` of the invoking webview's run process group plus a
/// sweep of every process still using its prefix. No active run (or a run
/// owned by another webview) is a no-op.
#[tauri::command]
pub async fn stop_run_executable(
    webview: tauri::Webview,
    app: AppState<'_>,
) -> Result<(), RunCommandError> {
    app.services()
        .run_executable
        .stop(&JobOwner::new(webview.label()));
    Ok(())
}
