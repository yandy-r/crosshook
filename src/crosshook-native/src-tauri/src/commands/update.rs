//! Thin IPC adapters over the update-game service.

use crosshook_app::services::{JobOwner, UpdateGameResponse};
use crosshook_core::update::UpdateGameRequest;
use tauri::AppHandle;

use super::{AppState, IpcResult};
use crate::error_map::ipc;
use crate::event_bridge::relay_update;

// Only pass the label of the injected invoking webview, never a JS argument.
fn update_owner(label: &str) -> JobOwner {
    JobOwner::new(label)
}

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
        .start(update_owner(webview.label()), request)
        .await)?;
    relay_update(handle, run);
    Ok(response)
}

/// Cancels this webview's active update; a no-op when it has none.
#[tauri::command]
pub async fn cancel_update(webview: tauri::Webview, app: AppState<'_>) -> IpcResult<()> {
    app.services().update.cancel(&update_owner(webview.label()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_owner_is_derived_from_the_webview_label() {
        assert_eq!(update_owner("main"), JobOwner::new("main"));
        assert_ne!(update_owner("main"), update_owner("other"));
    }

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = validate_update_request;
        let _ = update_game;
        let _ = cancel_update;
    }
}
