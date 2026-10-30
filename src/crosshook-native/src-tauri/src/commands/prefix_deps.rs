//! Thin IPC adapters over the prefix-dependency service.
//!
//! The service owns tool selection, the per-prefix job key, the supervised
//! install child, the Windows-version restore journal and its recovery. This
//! file maps its errors to strings and relays an install's events to the
//! webview with the raw request `profile_name`/`prefix_path` (listeners match
//! on the exact strings they sent).

use crosshook_app::services::prefix_deps::{
    completion_payload, DepCompletePayload, DepInstallCompletion, DepLogPayload,
    PrefixVersionRepairStatus,
};
use crosshook_app::services::{PrefixDepsService, ProcessRun, RunEvent};
use crosshook_app::AppError;
use crosshook_core::prefix_deps::{
    BinaryDetectionResult, PrefixDependencyInstallOutcome, PrefixDependencyStatus,
    PrefixVersionRestoreState,
};
use tauri::{AppHandle, Emitter as _, State};

use super::process_relay::batch_lines;

const PREFIX_DEP_LOG_EVENT: &str = "prefix-dep-log";
const PREFIX_DEP_COMPLETE_EVENT: &str = "prefix-dep-complete";

/// Shown when an install job ended without reporting a result.
const MSG_NO_RESULT: &str = "dependency install ended without a result";

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

/// The `prefix-dep-complete` payload of a finished install. A job that ended
/// without a completion is reported as a failed install whose restore is
/// still pending, so listeners stop waiting and the prefix is never reported
/// safe.
fn complete_payload(
    profile_name: &str,
    prefix_path: &str,
    completion: Option<DepInstallCompletion>,
) -> DepCompletePayload {
    let outcome = completion.map_or_else(
        || PrefixDependencyInstallOutcome {
            install_succeeded: false,
            install_exit_code: None,
            install_error: Some(MSG_NO_RESULT.to_string()),
            restore_state: PrefixVersionRestoreState::Pending,
            restore_error: Some(MSG_NO_RESULT.to_string()),
        },
        |done| done.outcome,
    );
    completion_payload(profile_name, prefix_path, outcome)
}

/// Relays the install's (already sanitized) log lines, then its completion.
/// The completion is emitted only after the service finished the job: final
/// log drain, restore and journal resolution included.
fn relay_install(
    app: AppHandle,
    profile_name: String,
    prefix_path: String,
    mut run: ProcessRun<DepInstallCompletion>,
) {
    drop(tauri::async_runtime::spawn(async move {
        while let Some(event) = run.next().await {
            match event {
                RunEvent::Log(batch) => {
                    for line in batch_lines(batch) {
                        let payload = DepLogPayload {
                            profile_name: profile_name.clone(),
                            prefix_path: prefix_path.clone(),
                            line,
                        };
                        if let Err(error) = app.emit(PREFIX_DEP_LOG_EVENT, &payload) {
                            tracing::warn!(%error, "failed to emit prefix-dep log line");
                        }
                    }
                }
                RunEvent::Finished(completion) => {
                    let payload = complete_payload(&profile_name, &prefix_path, completion);
                    if let Err(error) = app.emit(PREFIX_DEP_COMPLETE_EVENT, &payload) {
                        tracing::warn!(%error, "failed to emit prefix-dep completion");
                    }
                }
            }
        }
    }));
}

/// Detect the protontricks/winetricks binary.
#[tauri::command]
pub async fn detect_protontricks_binary(
    service: State<'_, PrefixDepsService>,
) -> Result<BinaryDetectionResult, String> {
    service.detect_binary().await.map_err(map_app_error)
}

/// Check which prefix dependencies are installed for a profile.
#[tauri::command]
pub async fn check_prefix_dependencies(
    profile_name: String,
    prefix_path: String,
    packages: Vec<String>,
    service: State<'_, PrefixDepsService>,
) -> Result<Vec<PrefixDependencyStatus>, String> {
    service
        .check(profile_name, prefix_path, packages)
        .await
        .map_err(map_app_error)
}

/// Start installing prefix dependencies; progress streams on `prefix-dep-log`,
/// then `prefix-dep-complete`.
#[tauri::command]
pub async fn install_prefix_dependency(
    profile_name: String,
    prefix_path: String,
    packages: Vec<String>,
    app: AppHandle,
    service: State<'_, PrefixDepsService>,
) -> Result<(), String> {
    let run = service
        .start_install(profile_name.clone(), prefix_path.clone(), packages)
        .await
        .map_err(map_app_error)?;
    relay_install(app, profile_name, prefix_path, run);
    Ok(())
}

/// Return whether a prefix has an interrupted or failed Windows-version restore.
#[tauri::command]
pub async fn get_prefix_version_repair_status(
    profile_name: String,
    prefix_path: String,
    service: State<'_, PrefixDepsService>,
) -> Result<PrefixVersionRepairStatus, String> {
    // Part of the frozen IPC arguments; the status is per prefix.
    drop(profile_name);
    service
        .repair_status(prefix_path)
        .await
        .map_err(map_app_error)
}

/// Retry and verify a persisted Windows-version restore plan.
#[tauri::command]
pub async fn repair_prefix_windows_version(
    profile_name: String,
    prefix_path: String,
    service: State<'_, PrefixDepsService>,
) -> Result<PrefixVersionRepairStatus, String> {
    // Part of the frozen IPC arguments; the repair is per prefix.
    drop(profile_name);
    service.repair(prefix_path).await.map_err(map_app_error)
}

/// Get cached dependency status for a profile.
#[tauri::command]
pub async fn get_dependency_status(
    profile_name: String,
    prefix_path: String,
    service: State<'_, PrefixDepsService>,
) -> Result<Vec<PrefixDependencyStatus>, String> {
    service
        .dependency_status(profile_name, prefix_path)
        .await
        .map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_app::services::prefix_deps::{LastRecovery, RecoveryOutcome};
    use serde::Serialize;

    fn assert_matches_fixture<T: Serialize>(value: &T, fixture: &str) {
        let actual = serde_json::to_value(value).expect("serialize payload");
        let expected: serde_json::Value =
            serde_json::from_str(fixture).expect("parse payload fixture");
        assert_eq!(actual, expected);
    }

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = detect_protontricks_binary;
        let _ = check_prefix_dependencies;
        let _ = install_prefix_dependency;
        let _ = get_prefix_version_repair_status;
        let _ = repair_prefix_windows_version;
        let _ = get_dependency_status;
    }

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("prefix_path", "prefix does not exist");
        assert_eq!(map_app_error(error), "prefix does not exist");
    }

    #[test]
    fn fixture_prefix_dep_log_wire_shape() {
        assert_matches_fixture(
            &DepLogPayload {
                profile_name: "p".to_string(),
                prefix_path: "/raw/prefix".to_string(),
                line: "l".to_string(),
            },
            include_str!("fixtures/process/prefix_dep_log.json"),
        );
    }

    #[test]
    fn fixture_prefix_dep_complete_wire_shape() {
        let completion = DepInstallCompletion {
            outcome: PrefixDependencyInstallOutcome {
                install_succeeded: true,
                install_exit_code: Some(0),
                install_error: None,
                restore_state: PrefixVersionRestoreState::Failed,
                restore_error: Some("e".to_string()),
            },
        };
        assert_matches_fixture(
            &complete_payload("p", "/raw/prefix", Some(completion)),
            include_str!("fixtures/process/prefix_dep_complete.json"),
        );
    }

    /// `pending` reaches the wire when the install child never ran or the
    /// job ended without a completion (fail closed: the row stays pending).
    #[test]
    fn fixture_prefix_dep_complete_pending_wire_shape() {
        assert_matches_fixture(
            &complete_payload("p", "/raw/prefix", None),
            include_str!("fixtures/process/prefix_dep_complete_pending.json"),
        );
    }

    /// The P0 fixture plus only the two additive runtime fields (D9).
    #[test]
    fn fixture_prefix_version_repair_status_wire_shape_is_additive() {
        let status = PrefixVersionRepairStatus {
            required: true,
            state: Some(PrefixVersionRestoreState::Pending),
            last_error: None,
            in_progress: false,
            last_recovery: None,
        };
        let mut expected: serde_json::Value = serde_json::from_str(include_str!(
            "fixtures/process/prefix_version_repair_status.json"
        ))
        .expect("parse payload fixture");
        let fields = expected.as_object_mut().expect("fixture is an object");
        assert!(fields
            .insert("in_progress".to_string(), serde_json::json!(false))
            .is_none());
        assert!(fields
            .insert("last_recovery".to_string(), serde_json::Value::Null)
            .is_none());
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
    }

    #[test]
    fn last_recovery_serializes_snake_case_outcome() {
        let recovery = LastRecovery {
            outcome: RecoveryOutcome::Restored,
            message: None,
        };
        assert_eq!(
            serde_json::to_value(&recovery).unwrap(),
            serde_json::json!({ "outcome": "restored", "message": null })
        );
    }

    #[test]
    fn complete_payload_keeps_raw_request_names() {
        let completion = DepInstallCompletion {
            outcome: PrefixDependencyInstallOutcome {
                install_succeeded: true,
                install_exit_code: Some(0),
                install_error: None,
                restore_state: PrefixVersionRestoreState::NotRequired,
                restore_error: None,
            },
        };
        let payload = complete_payload("My Game", "/games/my game/", Some(completion));
        assert_eq!(payload.profile_name, "My Game");
        assert_eq!(payload.prefix_path, "/games/my game/");
        assert!(payload.succeeded);
    }

    #[test]
    fn missing_completion_reports_an_unsafe_failed_install() {
        let payload = complete_payload("p", "/raw/prefix", None);
        assert_eq!(payload.profile_name, "p");
        assert_eq!(payload.prefix_path, "/raw/prefix");
        assert!(!payload.succeeded);
        assert!(!payload.install_succeeded);
        assert_eq!(payload.install_exit_code, None);
        assert_eq!(payload.restore_state, PrefixVersionRestoreState::Pending);
        assert_eq!(payload.restore_error.as_deref(), Some(MSG_NO_RESULT));
    }
}
