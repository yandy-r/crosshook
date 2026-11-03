use super::*;
use crosshook_app::services::launch::RunningSession;
use crosshook_core::launch::SessionId;
use serde_json::{json, Value};

const ID: &str = "550e8400-e29b-41d4-a716-446655440000";

fn session_id() -> SessionId {
    serde_json::from_value(json!(ID)).expect("session id")
}

fn payload(event: &AppEvent) -> Value {
    serde_json::to_value(LaunchSessionChanged::from_event(event).expect("bridged")).unwrap()
}

#[test]
fn event_name_is_launch_session_changed() {
    assert_eq!(LAUNCH_SESSION_CHANGED_EVENT, "launch-session-changed");
}

#[test]
fn started_and_exited_payloads_match_the_wire_contract() {
    let started = AppEvent::SessionStarted {
        session_id: session_id(),
        profile_name: Some("Alpha".into()),
        kind: SessionKind::Trainer,
        seq: 3,
    };
    assert_eq!(
        payload(&started),
        json!({
            "kind": "started",
            "session_id": ID,
            "profile_name": "Alpha",
            "session_kind": "trainer",
            "seq": 3,
        })
    );

    let exited = AppEvent::SessionExited {
        session_id: session_id(),
        profile_name: None,
        kind: SessionKind::Game,
        code: None,
        signal: Some(15),
        seq: 4,
    };
    assert_eq!(
        payload(&exited),
        json!({
            "kind": "exited",
            "session_id": ID,
            "profile_name": null,
            "session_kind": "game",
            "code": null,
            "signal": 15,
            "seq": 4,
        })
    );
}

#[test]
fn bus_resync_becomes_a_resync_marker_and_other_events_are_ignored() {
    assert_eq!(
        payload(&AppEvent::Resync { missed: 9 }),
        json!({ "kind": "resync" })
    );
    assert!(
        LaunchSessionChanged::from_event(&AppEvent::UiSettingsPersistFailed {
            message: "x".into()
        })
        .is_none()
    );
}

#[test]
fn running_sessions_snapshot_matches_the_wire_contract() {
    let dto = RunningSessionsDto::from(RunningSessions {
        sessions: vec![RunningSession {
            session_id: session_id(),
            profile_name: Some("Alpha".into()),
            kind: SessionKind::Game,
        }],
        seq: 4,
    });
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        json!({
            "sessions": [{ "session_id": ID, "profile_name": "Alpha", "session_kind": "game" }],
            "seq": 4,
        })
    );
}
