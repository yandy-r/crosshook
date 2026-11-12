//! Thin IPC adapters over the update-game service.

use crosshook_app::services::{JobOwner, UpdateGameResponse};
use crosshook_core::update::UpdateGameRequest;
use tauri::AppHandle;

use super::{AppState, IpcResult};
use crate::error_map::ipc;
use crate::event_bridge::relay_update;

// Only pass the label of the injected invoking webview, never a JS argument.
#[tauri::command]
pub async fn validate_update_request(
    request: UpdateGameRequest,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app.services().update.validate(request).await)
}

/// Starts the updater and relays its log lines, then its exit code.
#[tauri::command]
pub async fn update_game(
    handle: AppHandle,
    webview: tauri::Webview,
    request: UpdateGameRequest,
    app: AppState<'_>,
) -> IpcResult<UpdateGameResponse> {
    let (response, run) = ipc(app
        .services()
        .update
        .start(JobOwner::new(webview.label()), request)
        .await)?;
    relay_update(handle, run);
    Ok(response)
}

/// Cancels this webview's active update; a no-op when it has none.
#[tauri::command]
pub async fn cancel_update(webview: tauri::Webview, app: AppState<'_>) -> IpcResult<()> {
    app.services()
        .update
        .cancel(&JobOwner::new(webview.label()));
    Ok(())
}
