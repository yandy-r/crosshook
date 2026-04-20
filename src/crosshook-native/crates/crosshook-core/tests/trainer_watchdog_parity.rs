//! Integration tests for issue #230 — trainer watchdog cleanup parity.
//!
//! These exercise the [`LaunchSessionRegistry`] + [`WatchdogOutcome`]
//! coordination at the boundary the Tauri layer actually uses. Full
//! process-spawn + gamescope teardown isn't reachable in CI; the goal here is
//! to prove the 4 issue acceptance criteria at the abstraction that drives
//! those paths:
//!
//! 1. Trainer exit while game runs → only trainer session deregistered.
//! 2. Game exit while trainer runs → game broadcasts LinkedSessionExit to
//!    trainer's cancel channel.
//! 3. Teardown reason is embedded in the serialized DiagnosticReport.
//! 4. Each launch session owns an isolated cancel channel — trainer cleanup
//!    cannot reach into the game's broadcast path.

use crosshook_core::launch::diagnostics::{
    DiagnosticReport, ExitCodeInfo, FailureMode, MAX_DIAGNOSTIC_ENTRIES,
};
use crosshook_core::launch::{
    LaunchSessionRegistry, SessionKind, TeardownReason, ValidationSeverity, WatchdogOutcome,
};

fn fresh_report() -> DiagnosticReport {
    DiagnosticReport {
        severity: ValidationSeverity::Info,
        summary: "clean".to_string(),
        exit_info: ExitCodeInfo {
            code: Some(0),
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::CleanExit,
            description: "clean exit".to_string(),
            severity: ValidationSeverity::Info,
        },
        pattern_matches: vec![],
        suggestions: vec![],
        launch_method: "proton_run".to_string(),
        log_tail_path: None,
        analyzed_at: "2026-04-19T00:00:00Z".to_string(),
        teardown_reason: None,
    }
}

/// Acceptance criterion 1: trainer exit while game runs cleans up only the
/// trainer side — game session stays registered and receives no cancel.
#[tokio::test]
async fn trainer_exit_leaves_game_session_running() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, mut game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (trainer_id, _trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
    registry
        .link_to_parent(trainer_id, game_id)
        .expect("link ok");

    // Simulate: trainer process exits on its own. Stream finalizer will
    // deregister the trainer session. Game session is untouched.
    registry.deregister(trainer_id);

    // Sanity: the game session is still in the registry and has not received
    // a cancel signal from the trainer's teardown.
    assert!(
        game_rx.try_recv().is_err(),
        "game session must not receive a cancel from trainer exit"
    );

    // The game session's cancel channel is still live and can be used to
    // continue driving its watchdog. We tear it down at test-end to release
    // resources.
    registry.deregister(game_id);
}

/// Acceptance criterion 2: game exit while trainer runs cascades a
/// LinkedSessionExit broadcast to the trainer's cancel channel so the
/// trainer watchdog's `tokio::select!` fires and tears down the trainer
/// tree.
#[tokio::test]
async fn game_exit_cascades_linked_session_exit_to_trainer() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (trainer_id, mut trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
    registry
        .link_to_parent(trainer_id, game_id)
        .expect("link ok");

    // Simulate the stream finalizer for the game session. This is the same
    // helper call the Tauri-side finalize_launch_session performs for game
    // launches.
    let signalled = registry.cancel_linked_children(game_id, TeardownReason::LinkedSessionExit);
    assert_eq!(
        signalled, 1,
        "exactly one trainer should receive the cancel"
    );
    registry.deregister(game_id);

    let received = tokio::time::timeout(std::time::Duration::from_millis(100), trainer_rx.recv())
        .await
        .expect("trainer receives cancel within 100ms")
        .expect("recv ok");
    assert_eq!(
        received,
        TeardownReason::LinkedSessionExit,
        "trainer's watchdog cancel must be LinkedSessionExit"
    );

    // Trainer watchdog would now call shutdown_gamescope_tree(_, outcome,
    // LinkedSessionExit). Simulate that effect at the outcome level.
    let trainer_outcome = WatchdogOutcome::new();
    trainer_outcome.mark(TeardownReason::LinkedSessionExit);
    assert!(trainer_outcome.was_killed());
    assert_eq!(
        trainer_outcome.reason(),
        Some(TeardownReason::LinkedSessionExit)
    );

    registry.deregister(trainer_id);
}

/// Acceptance criterion 3: teardown reason lands in the serialized
/// `DiagnosticReport`. This is what [`record_launch_finished`] persists into
/// `launch_operations.diagnostic_json`.
#[test]
fn teardown_reason_round_trips_through_diagnostic_report_json() {
    let mut report = fresh_report();
    report.teardown_reason = Some(TeardownReason::LinkedSessionExit);

    let json = serde_json::to_string(&report).expect("serialize");
    assert!(
        json.contains("\"teardown_reason\":\"LinkedSessionExit\""),
        "teardown reason must be embedded verbatim in diagnostic_json: {json}"
    );

    let parsed: DiagnosticReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        parsed.teardown_reason,
        Some(TeardownReason::LinkedSessionExit)
    );
}

/// Backward compat: a pre-#230 `diagnostic_json` row without a
/// `teardown_reason` field still deserializes (the field is optional).
#[test]
fn legacy_diagnostic_report_without_teardown_reason_deserializes() {
    let legacy_json = r#"{
        "severity": "info",
        "summary": "legacy",
        "exit_info": {
            "code": 0,
            "signal": null,
            "signal_name": null,
            "core_dumped": false,
            "failure_mode": "clean_exit",
            "description": "ok",
            "severity": "info"
        },
        "pattern_matches": [],
        "suggestions": [],
        "launch_method": "proton_run",
        "log_tail_path": null,
        "analyzed_at": "2026-01-01T00:00:00Z"
    }"#;

    let parsed: DiagnosticReport =
        serde_json::from_str(legacy_json).expect("legacy rows must deserialize");
    assert_eq!(parsed.teardown_reason, None);
    let _ = MAX_DIAGNOSTIC_ENTRIES; // silence unused-import when tests compile out
}

/// Acceptance criterion 4: each session owns a distinct cancel channel.
/// Broadcasting to one session's channel never reaches another session's
/// receiver, and cancelling a session never kills a sibling.
#[tokio::test]
async fn sessions_have_isolated_cancel_channels() {
    let registry = LaunchSessionRegistry::new();
    let (game_a, mut game_a_rx) = registry.register(SessionKind::Game, "profile-a");
    let (game_b, mut game_b_rx) = registry.register(SessionKind::Game, "profile-b");
    let (trainer_a, mut trainer_a_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (trainer_b, mut trainer_b_rx) = registry.register(SessionKind::Trainer, "profile-b");

    registry.link_to_parent(trainer_a, game_a).expect("link a");
    registry.link_to_parent(trainer_b, game_b).expect("link b");

    // Finalize game_a — only trainer_a should receive the cascade.
    let signalled = registry.cancel_linked_children(game_a, TeardownReason::LinkedSessionExit);
    assert_eq!(signalled, 1);

    let cancel = tokio::time::timeout(std::time::Duration::from_millis(100), trainer_a_rx.recv())
        .await
        .expect("trainer_a timeout")
        .expect("trainer_a recv ok");
    assert_eq!(cancel, TeardownReason::LinkedSessionExit);

    assert!(
        trainer_b_rx.try_recv().is_err(),
        "trainer_b must NOT receive game_a's cancel"
    );
    assert!(
        game_a_rx.try_recv().is_err(),
        "game_a's own receiver is never signalled by its own cancel_linked_children call"
    );
    assert!(
        game_b_rx.try_recv().is_err(),
        "game_b is isolated from game_a teardown"
    );

    registry.deregister(game_a);
    registry.deregister(trainer_a);
    registry.deregister(game_b);
    registry.deregister(trainer_b);
}

/// Regression guard: `WatchdogOutcome::mark` keeps the first reason so a
/// race between the natural-exit path and a late LinkedSessionExit cancel
/// doesn't overwrite the authoritative teardown cause in diagnostics.
#[test]
fn watchdog_outcome_retains_first_reason() {
    let outcome = WatchdogOutcome::new();
    outcome.mark(TeardownReason::WatchdogNaturalExit);
    outcome.mark(TeardownReason::LinkedSessionExit);
    assert_eq!(outcome.reason(), Some(TeardownReason::WatchdogNaturalExit));
}

/// Trainer-without-gamescope drain path: when a parent-cascade cancel
/// arrives, the drain helper records the reason without flagging a kill
/// (there was no gamescope tree to tear down). The stream finalizer should
/// then stamp `teardown_reason` but leave the normal exit summary alone.
#[test]
fn record_reason_attributes_cascade_without_claiming_kill() {
    let outcome = WatchdogOutcome::new();
    outcome.record_reason(TeardownReason::LinkedSessionExit);

    // Simulate the finalizer's teardown_reason assignment.
    let teardown_reason = outcome.reason().or(Some(TeardownReason::NaturalExit));

    assert!(
        !outcome.was_killed(),
        "record_reason must not set killed so the finalizer skips the \
         gamescope-cleanup summary override"
    );
    assert_eq!(teardown_reason, Some(TeardownReason::LinkedSessionExit));
}

/// Distinct-variant guard: a Closed broadcast channel maps to
/// [`TeardownReason::ReceiverClosed`] (not `LinkedSessionExit`) so the
/// audit trail doesn't falsely attribute a cascade when the registry
/// entry merely got dropped.
#[test]
fn receiver_closed_serializes_distinctly_from_linked_session_exit() {
    let linked_json = serde_json::to_string(&TeardownReason::LinkedSessionExit).unwrap();
    let closed_json = serde_json::to_string(&TeardownReason::ReceiverClosed).unwrap();
    assert_ne!(linked_json, closed_json);
    assert_eq!(closed_json, "\"ReceiverClosed\"");
}
