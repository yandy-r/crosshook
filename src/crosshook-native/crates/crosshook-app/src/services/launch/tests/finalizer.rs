//! Finalizer side effects and the drop guard (Task 2.5).
//!
//! The history row, version snapshot and known-good tag follow the report
//! after the watchdog rewrite. `exit_code` and `signal` stay the raw status.
//! Trainer injections are published before `Exited`. A panic while the guard
//! is held still publishes one `Exited` and runs teardown.

use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Duration;

use crosshook_core::launch::{
    LaunchHookExecutionContext, LaunchSessionRegistry, SessionId, SessionKind, TeardownReason,
    WatchdogOutcome, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::{
    ConfigRevisionSource, LaunchHistoryEntry, MetadataStore, SyncSource,
    MAX_CONFIG_REVISIONS_PER_PROFILE,
};
use crosshook_core::profile::GameProfile;

use super::super::finalizer::SessionGuard;
use super::super::types::{
    EventPublisher, InjectionLevel, InjectionSource, LaunchEvent, SessionContext,
};
use super::helpers::{Harness, Metadata};

const PROFILE: &str = "finalizer-profile";
const COLLECT_TIMEOUT: Duration = Duration::from_secs(5);

struct LaunchCase {
    kind: SessionKind,
    method: &'static str,
    operation_id: Option<String>,
    outcome: WatchdogOutcome,
}

impl LaunchCase {
    fn new(kind: SessionKind, method: &'static str, operation_id: impl Into<String>) -> Self {
        Self {
            kind,
            method,
            operation_id: Some(operation_id.into()),
            outcome: WatchdogOutcome::new(),
        }
    }
}

fn seed_profile(metadata: &MetadataStore, name: &str) -> String {
    metadata
        .observe_profile_write(
            name,
            &GameProfile::default(),
            std::path::Path::new("/tmp/finalizer-profile.toml"),
            SyncSource::AppWrite,
            None,
        )
        .unwrap();
    metadata.lookup_profile_id(name).unwrap().unwrap()
}

fn seed_revision(metadata: &MetadataStore, profile_id: &str) {
    metadata
        .insert_config_revision(
            profile_id,
            PROFILE,
            ConfigRevisionSource::ManualSave,
            "hash-1",
            "toml = true",
            None,
            MAX_CONFIG_REVISIONS_PER_PROFILE,
        )
        .unwrap()
        .unwrap();
}

fn seeded(method: &str) -> (Harness, String, String) {
    let harness = Harness::new(Metadata::Enabled);
    let profile_id = seed_profile(&harness.metadata, PROFILE);
    seed_revision(&harness.metadata, &profile_id);
    let operation_id = harness
        .metadata
        .record_launch_started(Some(PROFILE), method, None)
        .unwrap();
    (harness, profile_id, operation_id)
}

fn session_context(
    harness: &Harness,
    registry: Arc<LaunchSessionRegistry>,
    session_id: SessionId,
    spec: &LaunchCase,
) -> SessionContext {
    let log_path = harness.root.path().join("launch.log");
    std::fs::write(&log_path, "").unwrap();
    SessionContext {
        session_id,
        kind: spec.kind,
        method: spec.method,
        profile_name: Some(PROFILE.to_string()),
        steam_app_id: String::new(),
        steam_client_path: String::new(),
        trainer_host_path: None,
        log_path,
        operation_id: spec.operation_id.clone(),
        watchdog_outcome: spec.outcome.clone(),
        coexistence: Vec::new(),
        post_exit_hooks: Vec::new(),
        hook_context: LaunchHookExecutionContext::default(),
        portal_guard: None,
        metadata: harness.metadata.clone(),
        registry,
    }
}

fn exit_code(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

async fn collect(events: &mut super::super::LaunchEvents) -> Vec<LaunchEvent> {
    let drain = async {
        let mut out = Vec::new();
        while let Some(event) = events.recv().await {
            let exited = matches!(event, LaunchEvent::Exited { .. });
            out.push(event);
            if exited {
                return out;
            }
        }
        panic!("event stream ended without Exited: {out:?}");
    };
    tokio::time::timeout(COLLECT_TIMEOUT, drain)
        .await
        .expect("Exited within 5s")
}

async fn finish(harness: &Harness, spec: LaunchCase, status: ExitStatus) -> Vec<LaunchEvent> {
    let registry = Arc::new(LaunchSessionRegistry::new());
    let (session_id, _cancel) = registry.register(spec.kind, PROFILE);
    let (publisher, mut events) = EventPublisher::channel(session_id);
    let ctx = session_context(harness, Arc::clone(&registry), session_id, &spec);
    SessionGuard::new(ctx, publisher).finish(Some(status)).await;

    let collected = collect(&mut events).await;
    let trailing = tokio::time::timeout(COLLECT_TIMEOUT, events.recv())
        .await
        .expect("stream closed after Exited");
    assert!(trailing.is_none(), "nothing is published after Exited");
    assert!(
        collected.iter().all(|event| event.session_id() == session_id),
        "every event carries the session id"
    );
    assert!(
        registry.sessions_for_profile(PROFILE, None).is_empty(),
        "session is deregistered"
    );
    collected
}

fn only_row(metadata: &MetadataStore) -> LaunchHistoryEntry {
    let rows = metadata
        .query_launch_history_for_profile(PROFILE, 10)
        .unwrap();
    assert_eq!(rows.len(), 1, "one history row");
    rows.into_iter().next().unwrap()
}

fn has_snapshot(metadata: &MetadataStore, profile_id: &str) -> bool {
    metadata
        .lookup_latest_version_snapshot(profile_id)
        .unwrap()
        .is_some()
}

fn is_known_good(metadata: &MetadataStore, profile_id: &str) -> bool {
    let revisions = metadata
        .list_config_revisions(profile_id, Some(1))
        .unwrap();
    assert_eq!(revisions.len(), 1, "the seeded revision is still there");
    revisions[0].is_last_known_working
}

fn assert_exited(events: &[LaunchEvent], code: Option<i32>, signal: Option<i32>) {
    let exit_count = events
        .iter()
        .filter(|event| matches!(event, LaunchEvent::Exited { .. }))
        .count();
    assert_eq!(exit_count, 1, "Exited is published once");
    match events.last() {
        Some(LaunchEvent::Exited {
            code: got_code,
            signal: got_signal,
            ..
        }) => {
            assert_eq!(*got_code, code);
            assert_eq!(*got_signal, signal);
        }
        other => panic!("last event is Exited, got {other:?}"),
    }
}

fn assert_injections(events: &[LaunchEvent], expected: &[(InjectionLevel, InjectionSource, &str)]) {
    let got: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            LaunchEvent::Injection { event, .. } => {
                Some((event.level, event.source, event.message.as_str()))
            }
            _ => None,
        })
        .collect();
    let expected: Vec<_> = expected
        .iter()
        .map(|(level, source, message)| (*level, *source, *message))
        .collect();
    assert_eq!(got, expected);
}

fn assert_order(events: &[LaunchEvent]) {
    let exit_at = events
        .iter()
        .position(|event| matches!(event, LaunchEvent::Exited { .. }))
        .expect("Exited");
    assert_eq!(exit_at, events.len() - 1, "nothing follows Exited");
    let mut injections_started = false;
    for event in &events[..exit_at] {
        match event {
            LaunchEvent::Diagnostic { .. } => {
                assert!(
                    !injections_started,
                    "Diagnostic precedes trainer injections"
                );
            }
            LaunchEvent::Injection { .. } => injections_started = true,
            other => panic!("unexpected event before Exited: {other:?}"),
        }
    }
}

fn surfaced(events: &[LaunchEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, LaunchEvent::Diagnostic { .. }))
}

#[tokio::test]
async fn proton_exit_zero_records_success_snapshot_and_known_good() {
    let (harness, profile_id, operation_id) = seeded(METHOD_PROTON_RUN);
    let events = finish(
        &harness,
        LaunchCase::new(SessionKind::Game, METHOD_PROTON_RUN, operation_id),
        exit_code(0),
    )
    .await;

    assert_exited(&events, Some(0), None);
    assert!(!surfaced(&events), "clean exit is not surfaced");
    assert_injections(&events, &[]);

    let row = only_row(&harness.metadata);
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.exit_code, Some(0));
    assert_eq!(row.signal, None);
    assert_eq!(row.failure_mode.as_deref(), Some("clean_exit"));
    assert!(has_snapshot(&harness.metadata, &profile_id));
    assert!(is_known_good(&harness.metadata, &profile_id));
}

#[tokio::test]
async fn proton_exit_one_records_failure_without_snapshot_or_tag() {
    let (harness, profile_id, operation_id) = seeded(METHOD_PROTON_RUN);
    let events = finish(
        &harness,
        LaunchCase::new(SessionKind::Game, METHOD_PROTON_RUN, operation_id),
        exit_code(1),
    )
    .await;

    assert_exited(&events, Some(1), None);
    assert!(surfaced(&events), "non-zero exit is surfaced");
    assert_order(&events);

    let row = only_row(&harness.metadata);
    assert_eq!(row.status, "failed");
    assert_eq!(row.exit_code, Some(1));
    assert_eq!(row.signal, None);
    assert!(!has_snapshot(&harness.metadata, &profile_id));
    assert!(!is_known_good(&harness.metadata, &profile_id));
}

#[tokio::test]
async fn signal_nine_records_failed_with_signal() {
    let (harness, profile_id, operation_id) = seeded(METHOD_PROTON_RUN);
    let events = finish(
        &harness,
        LaunchCase::new(SessionKind::Game, METHOD_PROTON_RUN, operation_id),
        ExitStatus::from_raw(9),
    )
    .await;

    assert_exited(&events, None, Some(9));
    let row = only_row(&harness.metadata);
    assert_eq!(row.status, "failed");
    assert_eq!(row.exit_code, None);
    assert_eq!(row.signal, Some(9));
    assert_eq!(row.failure_mode.as_deref(), Some("kill"));
    assert!(!has_snapshot(&harness.metadata, &profile_id));
    assert!(!is_known_good(&harness.metadata, &profile_id));
}

#[tokio::test]
async fn watchdog_killed_exit_143_succeeds_without_diagnostic() {
    let (harness, profile_id, operation_id) = seeded(METHOD_PROTON_RUN);
    let spec = LaunchCase::new(SessionKind::Trainer, METHOD_PROTON_RUN, operation_id);
    spec.outcome.mark(TeardownReason::WatchdogNaturalExit);

    let events = finish(&harness, spec, exit_code(143)).await;

    assert_exited(&events, Some(143), None);
    assert!(!surfaced(&events), "watchdog rewrite is a clean exit");
    assert_injections(
        &events,
        &[
            (
                InjectionLevel::Warning,
                InjectionSource::Runtime,
                "Trainer session teardown was recorded.",
            ),
            (
                InjectionLevel::Error,
                InjectionSource::Trainer,
                "Trainer launch ended with diagnostics.",
            ),
        ],
    );
    assert_order(&events);

    let row = only_row(&harness.metadata);
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.exit_code, Some(143), "history keeps the raw exit code");
    assert_eq!(row.signal, None);
    assert_eq!(row.failure_mode.as_deref(), Some("clean_exit"));
    assert!(has_snapshot(&harness.metadata, &profile_id));
    assert!(is_known_good(&harness.metadata, &profile_id));
}

#[tokio::test]
async fn steam_exit_zero_is_indeterminate() {
    let (harness, profile_id, operation_id) = seeded(METHOD_STEAM_APPLAUNCH);
    let events = finish(
        &harness,
        LaunchCase::new(SessionKind::Game, METHOD_STEAM_APPLAUNCH, operation_id),
        exit_code(0),
    )
    .await;

    assert_exited(&events, Some(0), None);
    assert!(
        !surfaced(&events),
        "indeterminate steam exit is not surfaced"
    );

    let row = only_row(&harness.metadata);
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.exit_code, Some(0));
    assert_eq!(row.failure_mode.as_deref(), Some("indeterminate"));
    assert!(has_snapshot(&harness.metadata, &profile_id));
    assert!(is_known_good(&harness.metadata, &profile_id));
}

#[tokio::test]
async fn disabled_metadata_still_publishes_exited_once() {
    let harness = Harness::new(Metadata::Disabled);
    let operation_id = harness
        .metadata
        .record_launch_started(Some(PROFILE), METHOD_PROTON_RUN, None)
        .unwrap();
    let events = finish(
        &harness,
        LaunchCase::new(SessionKind::Game, METHOD_PROTON_RUN, operation_id),
        exit_code(0),
    )
    .await;

    assert_eq!(events.len(), 1, "no diagnostic and no injection");
    assert_exited(&events, Some(0), None);
    assert!(
        harness
            .metadata
            .query_launch_history_for_profile(PROFILE, 10)
            .unwrap()
            .is_empty(),
        "disabled metadata records nothing"
    );
}

#[tokio::test]
async fn trainer_events_are_diagnostic_then_injections_then_exited() {
    let harness = Harness::new(Metadata::Enabled);
    let operation_id = harness
        .metadata
        .record_launch_started(Some(PROFILE), METHOD_PROTON_RUN, None)
        .unwrap();
    let events = finish(
        &harness,
        LaunchCase::new(SessionKind::Trainer, METHOD_PROTON_RUN, operation_id),
        exit_code(1),
    )
    .await;

    assert_exited(&events, Some(1), None);
    assert!(surfaced(&events));
    assert_injections(
        &events,
        &[( 
            InjectionLevel::Error,
            InjectionSource::Trainer,
            "Trainer launch ended with diagnostics.",
        )],
    );
    assert_order(&events);
    assert!(matches!(
        events.as_slice(),
        [
            LaunchEvent::Diagnostic { .. },
            LaunchEvent::Injection { .. },
            LaunchEvent::Exited { .. },
        ]
    ));
}

#[tokio::test]
async fn panic_still_publishes_exited_and_tears_down() {
    let harness = Harness::new(Metadata::Enabled);
    let registry = Arc::new(LaunchSessionRegistry::new());
    let (game_id, _game_cancel) = registry.register(SessionKind::Game, PROFILE);
    let (trainer_id, mut trainer_rx) = registry.register(SessionKind::Trainer, PROFILE);
    registry.link_to_parent(trainer_id, game_id).unwrap();

    let (publisher, mut events) = EventPublisher::channel(game_id);
    let ctx = session_context(
        &harness,
        Arc::clone(&registry),
        game_id,
        &LaunchCase::new(SessionKind::Game, METHOD_PROTON_RUN, String::new()),
    );
    let ctx = SessionContext {
        operation_id: None,
        ..ctx
    };

    let panicked = tokio::spawn(async move {
        let _guard = SessionGuard::new(ctx, publisher);
        panic!("session task panicked");
    });
    let join = tokio::time::timeout(COLLECT_TIMEOUT, panicked)
        .await
        .expect("panicked task finished");
    assert!(join.expect_err("session task panics").is_panic());

    let collected = collect(&mut events).await;
    assert_eq!(collected.len(), 1, "panic skips the report and wrap-up");
    assert_exited(&collected, None, None);
    let trailing = tokio::time::timeout(COLLECT_TIMEOUT, events.recv())
        .await
        .expect("stream closed after Exited");
    assert!(trailing.is_none(), "exactly one Exited");

    let reason = tokio::time::timeout(COLLECT_TIMEOUT, trainer_rx.recv())
        .await
        .expect("linked trainer notified")
        .expect("cancel channel open");
    assert_eq!(reason, TeardownReason::LinkedSessionExit);
    assert!(
        registry
            .sessions_for_profile(PROFILE, Some(SessionKind::Game))
            .is_empty(),
        "panicked game session is deregistered"
    );
    assert_eq!(
        registry
            .sessions_for_profile(PROFILE, Some(SessionKind::Trainer))
            .len(),
        1,
        "the linked trainer stays registered"
    );
}

