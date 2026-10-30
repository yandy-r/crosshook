//! Thin IPC adapters over the update-game service.

use crosshook_app::{
    services::{JobOwner, RunEvent, UpdateCompletion, UpdateGameResponse, UpdateService},
    AppError,
};
use crosshook_core::update::UpdateGameRequest;
use tauri::{AppHandle, Emitter as _, State};

use super::process_relay::batch_lines;

const UPDATE_LOG_EVENT: &str = "update-log";
const UPDATE_COMPLETE_EVENT: &str = "update-complete";

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

// Only pass the label of the injected invoking webview, never a JS argument.
fn update_owner(label: &str) -> JobOwner {
    JobOwner::new(label)
}

/// The frozen `update-complete` payload: the exit code, or `null` when the
/// updater was ended by a signal or never ran (the reason is in its log).
fn complete_payload(completion: Option<UpdateCompletion>) -> Option<i32> {
    completion.and_then(|completion| completion.code)
}

#[tauri::command]
pub async fn validate_update_request(
    request: UpdateGameRequest,
    service: State<'_, UpdateService>,
) -> Result<(), String> {
    service.validate(request).await.map_err(map_app_error)
}

/// Starts the updater and relays its log lines, then its exit code.
#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    webview: tauri::Webview,
    request: UpdateGameRequest,
    service: State<'_, UpdateService>,
) -> Result<UpdateGameResponse, String> {
    let (response, mut run) = service
        .start(update_owner(webview.label()), request)
        .await
        .map_err(map_app_error)?;
    drop(tauri::async_runtime::spawn(async move {
        while let Some(event) = run.next().await {
            match event {
                RunEvent::Log(batch) => {
                    for line in batch_lines(batch) {
                        if let Err(error) = app.emit(UPDATE_LOG_EVENT, line) {
                            tracing::warn!(%error, "failed to emit update log line");
                        }
                    }
                }
                RunEvent::Finished(completion) => {
                    if let Err(error) =
                        app.emit(UPDATE_COMPLETE_EVENT, complete_payload(completion))
                    {
                        tracing::warn!(%error, "failed to emit update completion");
                    }
                }
            }
        }
    }));
    Ok(response)
}

/// Cancels this webview's active update; a no-op when it has none.
#[tauri::command]
pub async fn cancel_update(
    webview: tauri::Webview,
    service: State<'_, UpdateService>,
) -> Result<(), String> {
    service.cancel(&update_owner(webview.label()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = validate_update_request;
        let _ = update_game;
        let _ = cancel_update;
    }

    #[test]
    fn complete_payload_serializes_as_a_bare_number_or_null() {
        let exited = complete_payload(Some(UpdateCompletion {
            code: Some(3),
            signal: None,
        }));
        assert_eq!(serde_json::to_value(exited).unwrap(), json!(3));

        let signalled = complete_payload(Some(UpdateCompletion {
            code: None,
            signal: Some(15),
        }));
        assert_eq!(serde_json::to_value(signalled).unwrap(), json!(null));

        assert_eq!(
            serde_json::to_value(complete_payload(None)).unwrap(),
            json!(null)
        );
    }

    #[test]
    fn update_owner_is_derived_from_the_webview_label() {
        assert_eq!(update_owner("main"), JobOwner::new("main"));
        assert_ne!(update_owner("main"), update_owner("other"));
    }

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("updater_path", "updater path is required");
        assert_eq!(map_app_error(error), "updater path is required");
    }
}
