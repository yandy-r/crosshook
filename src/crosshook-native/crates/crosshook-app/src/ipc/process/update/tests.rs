use super::*;
use crate::log_tail::LogBatch;
use serde_json::json;

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
fn log_lines_are_bare_strings_and_completion_is_a_bare_code() {
    let events = wire_events(RunEvent::Log(LogBatch::Lines(vec![
        "a".into(),
        String::new(),
        "b".into(),
    ])));
    assert_eq!(
        events,
        vec![
            WireEvent {
                name: "update-log",
                payload: "\"a\"".into()
            },
            WireEvent {
                name: "update-log",
                payload: "\"b\"".into()
            },
        ]
    );
    let done = wire_events(RunEvent::Finished(Some(UpdateCompletion {
        code: Some(3),
        signal: None,
    })));
    assert_eq!(
        done,
        vec![WireEvent {
            name: "update-complete",
            payload: "3".into()
        }]
    );
    let none = wire_events(RunEvent::Finished(None));
    assert_eq!(none[0].payload, "null");
}
