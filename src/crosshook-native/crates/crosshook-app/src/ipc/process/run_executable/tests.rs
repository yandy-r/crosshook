use super::*;
use crate::AppError;
use serde_json::json;

fn assert_matches_fixture(value: &RunCommandError, fixture: &str) {
    let actual = serde_json::to_value(value).expect("serialize RunCommandError");
    let expected: serde_json::Value =
        serde_json::from_str(fixture).expect("parse RunCommandError fixture");
    assert_eq!(actual, expected);
}

#[test]
fn fixture_run_command_error_validation_wire_shape() {
    assert_matches_fixture(
        &RunCommandError::from(RunExecutableValidationError::ExecutablePathMissing),
        include_str!("../../fixtures/process/run_command_error_validation.json"),
    );
}

#[test]
fn fixture_run_command_error_runtime_wire_shape() {
    assert_matches_fixture(
        &RunCommandError::Runtime {
            message: "boom".to_string(),
        },
        include_str!("../../fixtures/process/run_command_error_runtime.json"),
    );
}

#[test]
fn start_error_maps_onto_the_frozen_envelope() {
    assert_matches_fixture(
        &RunCommandError::from(RunExecutableStartError::Validation(
            RunExecutableValidationError::ExecutablePathMissing,
        )),
        include_str!("../../fixtures/process/run_command_error_validation.json"),
    );
    assert_matches_fixture(
        &RunCommandError::from(RunExecutableStartError::Failed(AppError::validation(
            "prefix_path",
            "boom",
        ))),
        include_str!("../../fixtures/process/run_command_error_runtime.json"),
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

#[test]
fn log_lines_become_one_event_each_tagged_with_the_run_id() {
    let events = wire_events(
        "job-1",
        RunEvent::Log(crate::log_tail::LogBatch::Lines(vec![
            "a".into(),
            String::new(),
            "b".into(),
        ])),
    );
    let values: Vec<_> = events.iter().map(WireEvent::value).collect();
    assert!(events
        .iter()
        .all(|event| event.name == "run-executable-log"));
    assert_eq!(
        values,
        vec![
            json!({ "run_id": "job-1", "line": "a" }),
            json!({ "run_id": "job-1", "line": "b" })
        ]
    );
}

#[test]
fn finished_emits_one_completion_event() {
    let events = wire_events("job-1", RunEvent::Finished(None));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].name, "run-executable-complete");
    assert_eq!(
        events[0].value(),
        json!({ "run_id": "job-1", "code": null, "signal": null })
    );
}
