use std::time::{Duration, UNIX_EPOCH};

use crosshook_app::log_tail::LogBatch;
use crosshook_app::services::launch::{
    InjectionLevel, InjectionLogData, InjectionSource, LaunchEvent,
};
use crosshook_core::launch::{DiagnosticReport, SessionKind};
use serde_json::{json, Value};

use super::payloads::{
    suppression_summary_line, transform_launch_log_line_for_ui, InjectionLogEvent,
    LaunchCompletePayload, LaunchLogRelayState,
};
use super::{map_event, RelayEmit};

fn login_fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/commands/launch/fixtures/events")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture readable"))
        .expect("fixture is JSON")
}

fn session() -> (crosshook_core::launch::SessionId, String) {
    // Fixtures use @SESSION@; reuse an id generated via a registry-free path:
    // deserialize a fixed UUID-backed SessionId.
    let id: crosshook_core::launch::SessionId =
        serde_json::from_value(json!("550e8400-e29b-41d4-a716-446655440000")).expect("session id");
    (id, id.to_string())
}

#[test]
fn launch_result_unchanged_matches_fixture() {
    // LaunchResult gains no session_id (S9); its wire shape is frozen.
    let result = crate::commands::launch::shared::LaunchResult {
        succeeded: true,
        message: "Trainer launch started.".to_string(),
        helper_log_path: "/tmp/crosshook-logs/trainer-x.log".to_string(),
        warnings: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        login_fixture("launch_result.json")
    );
}

fn strip_session(mut value: Value) -> Value {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("session_id");
    }
    value
}

#[test]
fn launch_log_payload_adds_session_id_to_old_string() {
    let (id, id_str) = session();
    let mut state = LaunchLogRelayState::default();
    let emits = map_event(
        &mut state,
        LaunchEvent::Log {
            session_id: id,
            batch: LogBatch::Lines(vec!["Permission denied".to_string()]),
        },
    );
    assert_eq!(emits.len(), 1);
    let value = emits[0].payload_value();
    let fixture = login_fixture("launch_log.json");
    assert_eq!(value["line"], fixture);
    assert_eq!(
        value,
        json!({"line": "Permission denied", "session_id": id_str})
    );
}

#[test]
fn launch_complete_matches_fixtures_minus_session_id() {
    let (id, id_str) = session();
    for (fixture, code, signal) in [
        ("launch_complete_code.json", Some(0), None),
        ("launch_complete_signal.json", None, Some(9)),
    ] {
        let mut state = LaunchLogRelayState::default();
        let emits = map_event(
            &mut state,
            LaunchEvent::Exited {
                session_id: id,
                code,
                signal,
            },
        );
        assert_eq!(emits.len(), 1, "{fixture}");
        let value = emits[0].payload_value();
        assert_eq!(value["session_id"], id_str);
        assert_eq!(strip_session(value), login_fixture(fixture), "{fixture}");
    }
    // (f) nulls are serialized, not omitted.
    let payload = LaunchCompletePayload {
        code: None,
        signal: None,
        session_id: id_str.clone(),
    };
    assert_eq!(
        serde_json::to_value(payload).unwrap(),
        json!({"code": null, "signal": null, "session_id": id_str})
    );
}

#[test]
fn launch_diagnostic_matches_fixture_minus_session_id() {
    let (id, id_str) = session();
    let report: DiagnosticReport =
        serde_json::from_value(login_fixture("launch_diagnostic.json")).expect("report");
    let fixture = login_fixture("launch_diagnostic.json");
    let mut state = LaunchLogRelayState::default();
    let emits = map_event(
        &mut state,
        LaunchEvent::Diagnostic {
            session_id: id,
            report: Box::new(report),
        },
    );
    assert_eq!(emits.len(), 1);
    let value = emits[0].payload_value();
    assert_eq!(value["session_id"], id_str);
    assert_eq!(strip_session(value), fixture);
}

#[test]
fn injection_log_matches_fixture_with_session_substituted() {
    let (id, id_str) = session();
    let mut data = InjectionLogData::new(
        Some("  Synthetic Quest  "),
        SessionKind::Trainer,
        InjectionLevel::Warning,
        InjectionSource::Injection,
        "DLL injection engine is not available; stored hook configuration was not applied.",
        true,
    );
    data.at = UNIX_EPOCH + Duration::from_secs(1_767_225_600); // 2026-01-01T00:00:00Z
    let mut state = LaunchLogRelayState::default();
    let emits = map_event(
        &mut state,
        LaunchEvent::Injection {
            session_id: id,
            event: data,
        },
    );
    assert_eq!(emits.len(), 1);
    let mut expected = login_fixture("injection_log.json");
    expected["session_id"] = json!(id_str);
    // Fixture timestamp uses +00:00; relay formats RFC3339 the same way.
    expected["timestamp"] = json!("2026-01-01T00:00:00+00:00");
    assert_eq!(emits[0].payload_value(), expected);
    // hook_id / hook_name omitted (I-54).
    let value = emits[0].payload_value();
    assert!(value.get("hook_id").is_none());
    assert!(value.get("hook_name").is_none());
}

#[test]
fn every_emitted_payload_carries_same_session_id() {
    let (id, id_str) = session();
    let mut state = LaunchLogRelayState::default();
    let report: DiagnosticReport =
        serde_json::from_value(login_fixture("launch_diagnostic.json")).expect("report");
    let mut data = InjectionLogData::new(
        Some("Synthetic Quest"),
        SessionKind::Trainer,
        InjectionLevel::Info,
        InjectionSource::Trainer,
        "Trainer launch requested.",
        false,
    );
    data.at = UNIX_EPOCH;
    let events = vec![
        LaunchEvent::Log {
            session_id: id,
            batch: LogBatch::Lines(vec!["hello".into()]),
        },
        LaunchEvent::Diagnostic {
            session_id: id,
            report: Box::new(report),
        },
        LaunchEvent::Injection {
            session_id: id,
            event: data,
        },
        LaunchEvent::Exited {
            session_id: id,
            code: Some(0),
            signal: None,
        },
    ];
    let mut names = Vec::new();
    for event in events {
        for emit in map_event(&mut state, event) {
            names.push(emit.event_name());
            let value = emit.payload_value();
            // injection-log already carried session_id; the others gain it.
            assert_eq!(value["session_id"], id_str, "{}", emit.event_name());
        }
    }
    assert_eq!(
        names,
        vec![
            "launch-log",
            "launch-diagnostic",
            "injection-log",
            "launch-complete"
        ]
    );
}

#[test]
fn xdg_backend_suppression_first_verbatim_second_notice_repeats_dropped_summary_before_complete() {
    let (id, _) = session();
    let mut state = LaunchLogRelayState::default();
    let line = "[gamescope] [[0;31mError[0m] [0;37mxdg_backend:[0m Compositor released us but we were not acquired. Oh no.";
    let batch = |state: &mut LaunchLogRelayState| {
        map_event(
            state,
            LaunchEvent::Log {
                session_id: id,
                batch: LogBatch::Lines(vec![line.to_string()]),
            },
        )
    };
    let first = batch(&mut state);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].payload_value()["line"], line);
    let second = batch(&mut state);
    assert_eq!(second.len(), 1);
    assert_eq!(
        second[0].payload_value()["line"],
        "[crosshook] Suppressing repeated gamescope xdg_backend console noise. The raw launch log still contains every line."
    );
    assert!(batch(&mut state).is_empty());
    assert_eq!(state.gamescope_xdg_backend_suppressed, 2);

    // Summary precedes Complete on Exited.
    let emits = map_event(
        &mut state,
        LaunchEvent::Exited {
            session_id: id,
            code: Some(0),
            signal: None,
        },
    );
    assert_eq!(emits.len(), 2);
    assert!(matches!(emits[0], RelayEmit::Log { .. }));
    assert!(emits[0].payload_value()["line"]
        .as_str()
        .unwrap()
        .contains("2 repeated gamescope xdg_backend lines"));
    assert!(matches!(emits[1], RelayEmit::Complete(_)));
}

#[test]
fn transform_and_summary_keep_old_behaviour() {
    // Ported from the deleted tests/log_relay.rs.
    let mut state = LaunchLogRelayState::default();
    state.gamescope_xdg_backend_suppressed = 329;
    let summary = suppression_summary_line(&state).expect("summary line");
    assert!(summary.contains("329 repeated gamescope xdg_backend lines"));

    let mut state = LaunchLogRelayState::default();
    let line = "xdg_backend: Compositor released us but we were not acquired";
    assert_eq!(
        transform_launch_log_line_for_ui(&mut state, line),
        vec![line]
    );
}

#[test]
fn blank_lines_not_emitted_and_dropped_becomes_one_visible_line() {
    let (id, id_str) = session();
    let mut state = LaunchLogRelayState::default();
    let emits = map_event(
        &mut state,
        LaunchEvent::Log {
            session_id: id,
            batch: LogBatch::Lines(vec![String::new(), "real".into(), String::new()]),
        },
    );
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].payload_value()["line"], "real");

    let emits = map_event(
        &mut state,
        LaunchEvent::Log {
            session_id: id,
            batch: LogBatch::Dropped { n: 7 },
        },
    );
    assert_eq!(emits.len(), 1);
    assert_eq!(
        emits[0].payload_value(),
        json!({"line": "[crosshook] Console skipped 7 log batches; the raw launch log is complete.", "session_id": id_str})
    );
}

#[test]
fn injection_event_trims_and_falls_back_to_unknown_profile() {
    let (id, _) = session();
    let mut data = InjectionLogData::new(
        Some("   "),
        SessionKind::Game,
        InjectionLevel::Info,
        InjectionSource::Runtime,
        "Trainer lifecycle complete.",
        false,
    );
    data.at = UNIX_EPOCH;
    // InjectionLogData::new already normalises, so build the wire event directly.
    let event = InjectionLogEvent::from_data(id, &data);
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["profile_name"], "Unknown profile");
    assert_eq!(value["session_kind"], "game");
    assert_eq!(value["level"], "info");
    assert_eq!(value["source"], "runtime");
    assert!(value.get("unsupported_runtime").is_none());
}
