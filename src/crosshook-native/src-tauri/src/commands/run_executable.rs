//! Thin IPC adapters over the run-executable service.
//!
//! The service owns validation, the supervised Proton child, cancel/stop and
//! throwaway-prefix cleanup. This file maps its errors onto the frozen
//! [`RunCommandError`] envelope and relays a run's events to the webview.

use crosshook_app::services::{
    JobOwner, ProcessRun, RunCompletion, RunEvent, RunExecutableResponse, RunExecutableService,
    RunExecutableStartError,
};
use crosshook_core::run_executable::{RunExecutableRequest, RunExecutableValidationError};
use serde::Serialize;
use tauri::{AppHandle, Emitter as _, State};

use super::process_relay::batch_lines;

const RUN_EXECUTABLE_LOG_EVENT: &str = "run-executable-log";
const RUN_EXECUTABLE_COMPLETE_EVENT: &str = "run-executable-complete";

/// Structured error envelope returned to the frontend by every `run_executable`
/// command. The frontend matches on `kind` and uses `field` for input-level
/// error placement, removing the need for any message-text parsing.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunCommandError {
    Validation {
        variant: RunExecutableValidationError,
        field: &'static str,
        message: String,
    },
    Runtime {
        message: String,
    },
}

impl From<RunExecutableValidationError> for RunCommandError {
    fn from(value: RunExecutableValidationError) -> Self {
        Self::Validation {
            field: value.field(),
            message: value.message(),
            variant: value,
        }
    }
}

impl From<RunExecutableStartError> for RunCommandError {
    fn from(value: RunExecutableStartError) -> Self {
        match value {
            RunExecutableStartError::Validation(variant) => Self::from(variant),
            RunExecutableStartError::Failed(error) => Self::Runtime {
                message: error.message().to_string(),
            },
        }
    }
}

/// `run-executable-log` payload: one log line of the run `run_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RunExecutableLogPayload {
    run_id: String,
    line: String,
}

/// `run-executable-complete` payload: how the run `run_id` ended. Both
/// fields are `null` when the process could not be spawned or supervised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RunExecutableCompletePayload {
    run_id: String,
    code: Option<i32>,
    signal: Option<i32>,
}

impl RunExecutableCompletePayload {
    fn new(run_id: String, completion: Option<RunCompletion>) -> Self {
        let (code, signal) = completion.map_or((None, None), |done| (done.code, done.signal));
        Self {
            run_id,
            code,
            signal,
        }
    }
}

// Only pass the label of the injected invoking webview, never a JS argument.
fn run_owner(webview: &tauri::Webview) -> JobOwner {
    JobOwner::new(webview.label())
}

/// Relays the run's log lines, then its completion, to the webview. The
/// completion is emitted only after the service has finished the run
/// (final log drain and throwaway-prefix cleanup included).
fn relay_run(app: AppHandle, mut run: ProcessRun<RunCompletion>) {
    drop(tauri::async_runtime::spawn(async move {
        let run_id = run.run_id().to_string();
        while let Some(event) = run.next().await {
            match event {
                RunEvent::Log(batch) => {
                    for line in batch_lines(batch) {
                        let payload = RunExecutableLogPayload {
                            run_id: run_id.clone(),
                            line,
                        };
                        if let Err(error) = app.emit(RUN_EXECUTABLE_LOG_EVENT, &payload) {
                            tracing::warn!(%error, "failed to emit run-executable log line");
                        }
                    }
                }
                RunEvent::Finished(completion) => {
                    let payload = RunExecutableCompletePayload::new(run_id.clone(), completion);
                    if let Err(error) = app.emit(RUN_EXECUTABLE_COMPLETE_EVENT, &payload) {
                        tracing::warn!(%error, "failed to emit run-executable completion");
                    }
                }
            }
        }
    }));
}

#[tauri::command]
pub async fn validate_run_executable_request(
    request: RunExecutableRequest,
    service: State<'_, RunExecutableService>,
) -> Result<(), RunCommandError> {
    service
        .validate(request)
        .await
        .map_err(RunCommandError::from)
}

/// Starts a supervised run for the invoking webview and streams its output
/// on `run-executable-log`, then `run-executable-complete`, both tagged with
/// the returned `run_id`.
#[tauri::command]
pub async fn run_executable(
    app: AppHandle,
    webview: tauri::Webview,
    request: RunExecutableRequest,
    service: State<'_, RunExecutableService>,
) -> Result<RunExecutableResponse, RunCommandError> {
    let (response, run) = service
        .start(run_owner(&webview), request)
        .await
        .map_err(RunCommandError::from)?;
    relay_run(app, run);
    Ok(response)
}

/// Polite cancellation of the invoking webview's run: `SIGTERM` to its
/// process group, `SIGKILL` after the grace period. No active run (or a run
/// owned by another webview) is a no-op.
#[tauri::command]
pub fn cancel_run_executable(
    webview: tauri::Webview,
    service: State<'_, RunExecutableService>,
) -> Result<(), RunCommandError> {
    service.cancel(&run_owner(&webview));
    Ok(())
}

/// Immediate `SIGKILL` of the invoking webview's run process group plus a
/// sweep of every process still using its prefix. No active run (or a run
/// owned by another webview) is a no-op.
#[tauri::command]
pub fn stop_run_executable(
    webview: tauri::Webview,
    service: State<'_, RunExecutableService>,
) -> Result<(), RunCommandError> {
    service.stop(&run_owner(&webview));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_app::AppError;
    use serde_json::json;

    fn assert_matches_fixture(value: &RunCommandError, fixture: &str) {
        let actual = serde_json::to_value(value).expect("serialize RunCommandError");
        let expected: serde_json::Value =
            serde_json::from_str(fixture).expect("parse RunCommandError fixture");
        assert_eq!(actual, expected);
    }

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = validate_run_executable_request;
        let _ = run_executable;
        let _ = cancel_run_executable;
        let _ = stop_run_executable;
    }

    #[test]
    fn fixture_run_command_error_validation_wire_shape() {
        assert_matches_fixture(
            &RunCommandError::from(RunExecutableValidationError::ExecutablePathMissing),
            include_str!("fixtures/process/run_command_error_validation.json"),
        );
    }

    #[test]
    fn fixture_run_command_error_runtime_wire_shape() {
        assert_matches_fixture(
            &RunCommandError::Runtime {
                message: "boom".to_string(),
            },
            include_str!("fixtures/process/run_command_error_runtime.json"),
        );
    }

    #[test]
    fn start_error_maps_onto_the_frozen_envelope() {
        assert_matches_fixture(
            &RunCommandError::from(RunExecutableStartError::Validation(
                RunExecutableValidationError::ExecutablePathMissing,
            )),
            include_str!("fixtures/process/run_command_error_validation.json"),
        );
        assert_matches_fixture(
            &RunCommandError::from(RunExecutableStartError::Failed(AppError::validation(
                "prefix_path",
                "boom",
            ))),
            include_str!("fixtures/process/run_command_error_runtime.json"),
        );
    }

    #[test]
    fn log_payload_wire_shape() {
        let payload = RunExecutableLogPayload {
            run_id: "job-1".to_string(),
            line: "hello".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&payload).expect("serialize log payload"),
            json!({ "run_id": "job-1", "line": "hello" })
        );
    }

    #[test]
    fn complete_payload_wire_shape() {
        let exited = RunExecutableCompletePayload::new(
            "job-1".to_string(),
            Some(RunCompletion {
                code: Some(0),
                signal: None,
            }),
        );
        assert_eq!(
            serde_json::to_value(&exited).expect("serialize complete payload"),
            json!({ "run_id": "job-1", "code": 0, "signal": null })
        );

        let killed = RunExecutableCompletePayload::new(
            "job-2".to_string(),
            Some(RunCompletion {
                code: None,
                signal: Some(9),
            }),
        );
        assert_eq!(
            serde_json::to_value(&killed).expect("serialize complete payload"),
            json!({ "run_id": "job-2", "code": null, "signal": 9 })
        );

        let unsupervised = RunExecutableCompletePayload::new("job-3".to_string(), None);
        assert_eq!(
            serde_json::to_value(&unsupervised).expect("serialize complete payload"),
            json!({ "run_id": "job-3", "code": null, "signal": null })
        );
    }
}
