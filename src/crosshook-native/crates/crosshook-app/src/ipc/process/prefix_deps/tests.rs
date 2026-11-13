use super::*;
use crate::services::prefix_deps::{LastRecovery, PrefixVersionRepairStatus, RecoveryOutcome};
use serde::Serialize;

fn assert_matches_fixture<T: Serialize>(value: &T, fixture: &str) {
    let actual = serde_json::to_value(value).expect("serialize payload");
    let expected: serde_json::Value = serde_json::from_str(fixture).expect("parse payload fixture");
    assert_eq!(actual, expected);
}

#[test]
fn fixture_prefix_dep_log_wire_shape() {
    assert_matches_fixture(
        &DepLogPayload {
            profile_name: "p".to_string(),
            prefix_path: "/raw/prefix".to_string(),
            line: "l".to_string(),
        },
        include_str!("../../fixtures/process/prefix_dep_log.json"),
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
        include_str!("../../fixtures/process/prefix_dep_complete.json"),
    );
}

/// `pending` reaches the wire when the install child never ran or the
/// job ended without a completion (fail closed: the row stays pending).
#[test]
fn fixture_prefix_dep_complete_pending_wire_shape() {
    assert_matches_fixture(
        &complete_payload("p", "/raw/prefix", None),
        include_str!("../../fixtures/process/prefix_dep_complete_pending.json"),
    );
}

/// The P0 fixture plus only the two additive runtime fields.
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
        "../../fixtures/process/prefix_version_repair_status.json"
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

#[test]
fn log_and_completion_events_carry_the_raw_request_names() {
    let logs = wire_events(
        "My Game",
        "/games/my game/",
        RunEvent::Log(crate::log_tail::LogBatch::Lines(vec!["l".into()])),
    );
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].name, "prefix-dep-log");
    assert_eq!(logs[0].value()["profile_name"], "My Game");
    assert_eq!(logs[0].value()["prefix_path"], "/games/my game/");
    let done = wire_events("My Game", "/games/my game/", RunEvent::Finished(None));
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].name, "prefix-dep-complete");
    assert_eq!(done[0].value()["prefix_path"], "/games/my game/");
}
