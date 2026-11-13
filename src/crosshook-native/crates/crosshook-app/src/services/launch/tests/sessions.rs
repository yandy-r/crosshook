//! Running-session events: every registry change reaches the app
//! bus once, in `seq` order, and a subscribe-then-snapshot client applying
//! the events with `seq > snapshot.seq` always ends at the registry state.

use std::collections::BTreeSet;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crosshook_core::launch::{
    LaunchSessionRegistry, SessionKind, SessionObserver, METHOD_PROTON_RUN,
};

use super::super::finalizer::SessionGuard;
use super::super::sessions::session_observer;
use super::super::types::{EventPublisher, SessionContext};
use super::helpers::{exit_code, session_context, Harness, LaunchCase, Metadata};
use crate::events::{AppEvent, EventBus, EventReceiver};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// The next session event on `rx`, skipping unrelated bus traffic.
async fn next_session_event(rx: &mut EventReceiver) -> AppEvent {
    tokio::time::timeout(EVENT_TIMEOUT, async {
        loop {
            let event = rx.recv().await.expect("bus stays open");
            if matches!(
                event,
                AppEvent::SessionStarted { .. } | AppEvent::SessionExited { .. }
            ) {
                return event;
            }
        }
    })
    .await
    .expect("session event within 5 s")
}

async fn session_events(rx: &mut EventReceiver, count: usize) -> Vec<AppEvent> {
    let mut events = Vec::with_capacity(count);
    for _ in 0..count {
        events.push(next_session_event(rx).await);
    }
    events
}

fn seq_of(event: &AppEvent) -> u64 {
    match event {
        AppEvent::SessionStarted { seq, .. } | AppEvent::SessionExited { seq, .. } => *seq,
        other => panic!("not a session event: {other:?}"),
    }
}

/// Session ids of a snapshot, as an ordered set.
fn ids(sessions: &[crosshook_core::launch::SessionInfo]) -> BTreeSet<String> {
    sessions
        .iter()
        .map(|info| info.session_id.to_string())
        .collect()
}

/// The client protocol: apply events with `seq > snapshot_seq` on top of the
/// snapshot. A double start or an exit of an unknown session is a bug.
fn apply_after(
    mut state: BTreeSet<String>,
    snapshot_seq: u64,
    events: &[AppEvent],
) -> BTreeSet<String> {
    for event in events.iter().filter(|event| seq_of(event) > snapshot_seq) {
        match event {
            AppEvent::SessionStarted { session_id, .. } => {
                assert!(
                    state.insert(session_id.to_string()),
                    "double start: {event:?}"
                );
            }
            AppEvent::SessionExited { session_id, .. } => {
                assert!(
                    state.remove(&session_id.to_string()),
                    "unknown exit: {event:?}"
                );
            }
            other => panic!("not a session event: {other:?}"),
        }
    }
    state
}

/// Publishes a marker so a test can assert nothing else was queued before it.
async fn assert_no_more_session_events(bus: &EventBus, rx: &mut EventReceiver) {
    bus.publish(AppEvent::Resync { missed: 0 });
    let next = tokio::time::timeout(EVENT_TIMEOUT, rx.recv())
        .await
        .expect("marker within 5 s");
    assert_eq!(
        next,
        Some(AppEvent::Resync { missed: 0 }),
        "unexpected extra event"
    );
}

#[tokio::test]
async fn two_subscribers_both_see_started_then_exited_with_the_exit_code() {
    let harness = Harness::new(Metadata::Enabled);
    let mut first = harness.events.subscribe();
    let mut second = harness.events.subscribe();

    let session = harness
        .service
        .start_game(harness.proton_request("two-subscribers"))
        .await
        .expect("game starts");
    let id = session.id;
    Harness::collect(session).await;

    for rx in [&mut first, &mut second] {
        let events = session_events(rx, 2).await;
        assert_eq!(
            events,
            vec![
                AppEvent::SessionStarted {
                    session_id: id,
                    profile_name: Some("two-subscribers".to_string()),
                    kind: SessionKind::Game,
                    seq: 1,
                },
                AppEvent::SessionExited {
                    session_id: id,
                    profile_name: Some("two-subscribers".to_string()),
                    kind: SessionKind::Game,
                    code: Some(0),
                    signal: None,
                    seq: 2,
                },
            ]
        );
    }
    assert!(harness
        .service
        .running_sessions_snapshot()
        .sessions
        .is_empty());
}

fn guard_context(harness: &Harness, kind: SessionKind) -> (SessionContext, EventPublisher) {
    let registry = Arc::clone(harness.service.registry());
    let (session_id, _cancel) = registry.register(kind, "finalizer-profile");
    let ctx = session_context(
        harness,
        registry,
        session_id,
        &LaunchCase::new(kind, METHOD_PROTON_RUN, String::new()),
    );
    let (publisher, _events) = EventPublisher::channel(session_id);
    (
        SessionContext {
            operation_id: None,
            ..ctx
        },
        publisher,
    )
}

#[tokio::test]
async fn finish_and_its_drop_publish_exactly_one_exit_with_the_real_status() {
    let harness = Harness::new(Metadata::Enabled);
    let mut rx = harness.events.subscribe();

    let (ctx, publisher) = guard_context(&harness, SessionKind::Game);
    let finished = ctx.session_id;
    // `finish` consumes the guard, so its `Drop` runs right after.
    SessionGuard::new(ctx, publisher)
        .finish(Some(exit_code(3)))
        .await;

    let (ctx, publisher) = guard_context(&harness, SessionKind::Trainer);
    let signalled = ctx.session_id;
    SessionGuard::new(ctx, publisher)
        .finish(Some(ExitStatus::from_raw(15)))
        .await;

    let (ctx, publisher) = guard_context(&harness, SessionKind::Game);
    let dropped = ctx.session_id;
    drop(SessionGuard::new(ctx, publisher));

    let exits: Vec<_> = session_events(&mut rx, 6)
        .await
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::SessionExited {
                session_id,
                code,
                signal,
                ..
            } => Some((session_id, code, signal)),
            _ => None,
        })
        .collect();
    assert_eq!(
        exits,
        vec![
            (finished, Some(3), None),
            (signalled, None, Some(15)),
            (dropped, None, None),
        ]
    );
    assert_no_more_session_events(&harness.events, &mut rx).await;
}

#[tokio::test]
async fn spawn_failure_publishes_started_then_exited_without_status() {
    let harness = Harness::new(Metadata::Enabled);
    let mut rx = harness.events.subscribe();
    harness
        .spawner
        .fail_next(io::Error::other("scripted spawn failure"));

    harness
        .service
        .start_game(harness.proton_request("spawn-failure"))
        .await
        .expect_err("spawn fails");

    let events = session_events(&mut rx, 2).await;
    let AppEvent::SessionStarted { session_id, .. } = events[0] else {
        panic!("Started first: {events:?}");
    };
    assert_eq!(
        events[1],
        AppEvent::SessionExited {
            session_id,
            profile_name: Some("spawn-failure".to_string()),
            kind: SessionKind::Game,
            code: None,
            signal: None,
            seq: 2,
        }
    );
    let snapshot = harness.service.running_sessions_snapshot();
    assert!(snapshot.sessions.is_empty());
    assert_eq!(snapshot.seq, 2);
}

/// Dropping `start_game` after the session is registered but before the
/// session task's guard exists (it is parked on the history-row write) still
/// deregisters the session with exactly one `SessionExited`.
#[tokio::test]
async fn a_start_cancelled_after_registration_still_publishes_one_exit() {
    use std::future::Future;
    use std::task::Poll;

    let harness = Harness::new(Metadata::Enabled);
    let mut rx = harness.events.subscribe();
    let mut start = Box::pin(
        harness
            .service
            .start_game(harness.proton_request("cancelled-start")),
    );

    // Poll until the session is registered; the only await between
    // registration and the guard is `record_launch_started`.
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;
    loop {
        let registered = std::future::poll_fn(|cx| {
            if start.as_mut().poll(cx).is_ready() {
                panic!("start_game finished before it could be cancelled");
            }
            Poll::Ready(
                !harness
                    .service
                    .running_sessions_snapshot()
                    .sessions
                    .is_empty(),
            )
        })
        .await;
        if registered {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session never registered"
        );
        tokio::task::yield_now().await;
    }
    drop(start);

    let events = session_events(&mut rx, 2).await;
    let AppEvent::SessionStarted { session_id, .. } = events[0] else {
        panic!("Started first: {events:?}");
    };
    assert_eq!(
        events[1],
        AppEvent::SessionExited {
            session_id,
            profile_name: Some("cancelled-start".to_string()),
            kind: SessionKind::Game,
            code: None,
            signal: None,
            seq: 2,
        }
    );
    assert!(harness
        .service
        .running_sessions_snapshot()
        .sessions
        .is_empty());
    assert_no_more_session_events(&harness.events, &mut rx).await;
}

/// A change parks inside the observer (registry lock held) while a second
/// writer and a snapshot reader start. Neither may complete before the
/// parked change is published: otherwise the bus could deliver `seq` out of
/// order, or a snapshot could include a change whose event is still pending.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_snapshot_racing_an_in_flight_change_matches_the_event_stream() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let publish = session_observer(bus.clone());
    let (parked_tx, parked_rx) = mpsc::channel::<()>();
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let gate = Mutex::new(Some((parked_tx, gate_rx)));
    let observer: SessionObserver = Arc::new(move |change| {
        let first = gate.lock().unwrap().take();
        if let Some((parked, release)) = first {
            parked.send(()).unwrap();
            release.recv().unwrap();
        }
        publish(change);
    });
    let registry = Arc::new(LaunchSessionRegistry::with_observer(observer));

    let writer_a = {
        let registry = Arc::clone(&registry);
        thread::spawn(move || registry.register(SessionKind::Game, "alpha"))
    };
    parked_rx
        .recv_timeout(EVENT_TIMEOUT)
        .expect("alpha's change is in flight");

    let (done_tx, done_rx) = mpsc::channel::<&'static str>();
    let writer_b = {
        let registry = Arc::clone(&registry);
        let done = done_tx.clone();
        thread::spawn(move || {
            let registered = registry.register(SessionKind::Trainer, "beta");
            done.send("writer").unwrap();
            registered
        })
    };
    let reader = {
        let registry = Arc::clone(&registry);
        thread::spawn(move || {
            let snapshot = registry.snapshot();
            done_tx.send("reader").unwrap();
            snapshot
        })
    };
    assert!(
        done_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "a mutation or snapshot completed while another change was unpublished"
    );
    gate_tx.send(()).unwrap();

    let (alpha, _alpha_rx) = writer_a.join().unwrap();
    let (beta, _beta_rx) = writer_b.join().unwrap();
    let (snapshot, snapshot_seq) = reader.join().unwrap();
    registry.deregister(alpha);

    let events = session_events(&mut rx, 3).await;
    assert_eq!(events.iter().map(seq_of).collect::<Vec<_>>(), vec![1, 2, 3]);
    let (final_sessions, final_seq) = registry.snapshot();
    assert_eq!(final_seq, 3);
    assert_eq!(ids(&final_sessions), BTreeSet::from([beta.to_string()]));
    assert_eq!(
        apply_after(ids(&snapshot), snapshot_seq, &events),
        ids(&final_sessions)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_changes_reach_the_bus_gap_free_and_consistent_with_snapshots() {
    const WRITERS: usize = 4;
    const ROUNDS: usize = 25;
    let harness = Harness::new(Metadata::Disabled);
    let mut rx = harness.events.subscribe();
    let registry = Arc::clone(harness.service.registry());

    let writers: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let mut kept = Vec::new();
                for round in 0..ROUNDS {
                    let (id, cancel) = registry.register(SessionKind::Game, format!("w{writer}"));
                    if round % 5 == 0 {
                        kept.push(cancel);
                    } else {
                        registry.deregister(id);
                    }
                }
                kept
            })
        })
        .collect();
    let reader = {
        let registry = Arc::clone(&registry);
        thread::spawn(move || (0..100).map(|_| registry.snapshot()).collect::<Vec<_>>())
    };
    let _kept: Vec<_> = writers
        .into_iter()
        .flat_map(|writer| writer.join().unwrap())
        .collect();
    let snapshots = reader.join().unwrap();

    // 100 starts + 80 exits: under the bus capacity, so nothing lags.
    let total = WRITERS * ROUNDS + WRITERS * (ROUNDS - ROUNDS / 5);
    let events = session_events(&mut rx, total).await;
    let seqs: Vec<u64> = events.iter().map(seq_of).collect();
    assert_eq!(seqs, (1..=total as u64).collect::<Vec<_>>());

    let (final_sessions, final_seq) = registry.snapshot();
    assert_eq!(final_seq, total as u64);
    assert_eq!(final_sessions.len(), WRITERS * ROUNDS / 5);
    for (sessions, seq) in &snapshots {
        assert_eq!(
            apply_after(ids(sessions), *seq, &events),
            ids(&final_sessions),
            "snapshot at seq {seq} plus later events diverged"
        );
    }
}

#[tokio::test]
async fn a_lagging_subscriber_gets_resync_and_recovers_from_the_snapshot() {
    let bus = EventBus::new(2).unwrap();
    let mut rx = bus.subscribe();
    let registry = LaunchSessionRegistry::with_observer(session_observer(bus));
    let _sessions: Vec<_> = (0..5)
        .map(|n| registry.register(SessionKind::Game, format!("lag-{n}")))
        .collect();

    assert_eq!(rx.recv().await, Some(AppEvent::Resync { missed: 3 }));
    let (sessions, seq) = registry.snapshot();
    assert_eq!((sessions.len(), seq), (5, 5));
    // The retained tail is already reflected in the re-fetched snapshot.
    let tail = session_events(&mut rx, 2).await;
    assert_eq!(apply_after(ids(&sessions), seq, &tail), ids(&sessions));
}
