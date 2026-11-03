//! Steam game exit watch (YAN-824, D3). Unit tests drive
//! [`watch_steam_game`] with a scripted probe on paused Tokio time; one
//! service-level test checks the finalizer handoff end to end.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshook_core::launch::{
    LaunchSessionRegistry, SessionId, SessionKind, TeardownReason, WatchdogOutcome,
};
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;

use super::super::finalizer::SessionGuard;
use super::super::steam_watch::{
    steam_game_exe, watch_steam_game, ProcessProbe, SteamWatch, APPEAR_TIMEOUT, POLL_INTERVAL,
};
use super::super::types::{EventPublisher, SessionContext};
use super::helpers::{exit_code, session_context, Harness, LaunchCase, Metadata, Script};
use crate::events::AppEvent;

/// Probe answering from a script, repeating the last answer when it runs out.
#[derive(Clone)]
struct ScriptedProbe {
    answers: Arc<Mutex<VecDeque<bool>>>,
    calls: Arc<AtomicUsize>,
}

impl ScriptedProbe {
    fn new(answers: &[bool]) -> Self {
        Self {
            answers: Arc::new(Mutex::new(answers.iter().copied().collect())),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn probe(&self) -> ProcessProbe {
        let this = self.clone();
        Arc::new(move |exe: String| {
            assert_eq!(exe, "Game.exe");
            this.calls.fetch_add(1, Ordering::SeqCst);
            let mut answers = this.answers.lock().unwrap();
            let visible = if answers.len() > 1 {
                answers.pop_front().unwrap()
            } else {
                answers.front().copied().unwrap_or(false)
            };
            Box::pin(std::future::ready(visible))
        })
    }
}

struct Running {
    registry: Arc<LaunchSessionRegistry>,
    id: SessionId,
    outcome: WatchdogOutcome,
    handoff: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl Running {
    fn registered(&self) -> bool {
        self.registry
            .snapshot()
            .0
            .iter()
            .any(|info| info.session_id == self.id)
    }

    fn hand_off(&mut self) {
        self.handoff.take().unwrap().send(()).unwrap();
    }
}

fn start_watch(probe: &ScriptedProbe) -> Running {
    let registry = Arc::new(LaunchSessionRegistry::new());
    let (id, cancel_rx): (SessionId, broadcast::Receiver<TeardownReason>) =
        registry.register(SessionKind::Game, "steam-profile");
    let outcome = WatchdogOutcome::new();
    let (handoff, handoff_rx) = oneshot::channel();
    let watch = SteamWatch {
        session_id: id,
        exe_name: "Game.exe".to_string(),
        registry: Arc::clone(&registry),
        outcome: outcome.clone(),
        probe: probe.probe(),
    };
    let task = tokio::spawn(watch_steam_game(watch, cancel_rx, handoff_rx));
    Running {
        registry,
        id,
        outcome,
        handoff: Some(handoff),
        task,
    }
}

/// Lets the watcher run until it parks on a timer or a channel.
async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

async fn advance(by: Duration) {
    tokio::time::advance(by).await;
    settle().await;
}

#[tokio::test(start_paused = true)]
async fn deregisters_only_after_the_game_appeared_and_then_disappeared() {
    // Not visible twice, visible twice, then gone.
    let probe = ScriptedProbe::new(&[false, false, true, true, false]);
    let mut running = start_watch(&probe);
    settle().await;
    assert_eq!(probe.calls(), 0, "no probing while the helper still runs");

    running.hand_off();
    settle().await;
    for _ in 0..3 {
        assert!(running.registered());
        advance(POLL_INTERVAL).await;
    }
    assert!(running.registered(), "visible game keeps the session");
    advance(POLL_INTERVAL).await;
    assert!(!running.registered(), "deregistered once the game is gone");
    assert_eq!(probe.calls(), 5);
    running.task.await.unwrap();
    assert!(!running.outcome.was_killed(), "the watch never kills");
}

#[tokio::test(start_paused = true)]
async fn a_game_that_never_appears_is_deregistered_at_the_timeout() {
    let probe = ScriptedProbe::new(&[false]);
    let mut running = start_watch(&probe);
    running.hand_off();
    settle().await;

    advance(APPEAR_TIMEOUT - POLL_INTERVAL).await;
    assert!(running.registered(), "still waiting before the timeout");
    advance(POLL_INTERVAL).await;
    assert!(!running.registered(), "deregistered at the timeout");
    running.task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_reset_while_the_game_runs_deregisters_immediately() {
    let probe = ScriptedProbe::new(&[true]);
    let mut running = start_watch(&probe);
    running.hand_off();
    advance(POLL_INTERVAL * 3).await;
    assert!(running.registered());

    let calls = probe.calls();
    assert!(running
        .registry
        .cancel_session(running.id, TeardownReason::UserRequest));
    settle().await;
    assert!(!running.registered(), "reset ends the session at once");
    assert_eq!(probe.calls(), calls, "no probe after the reset");
    assert_eq!(running.outcome.reason(), Some(TeardownReason::UserRequest));
    assert!(!running.outcome.was_killed(), "the watch never kills");
    running.task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_reset_before_the_helper_exits_deregisters_at_handoff_without_probing() {
    let probe = ScriptedProbe::new(&[true]);
    let mut running = start_watch(&probe);
    assert!(running
        .registry
        .cancel_session(running.id, TeardownReason::UserRequest));
    settle().await;
    assert!(
        running.registered(),
        "the helper still runs; the finalizer owns it"
    );
    assert_eq!(running.outcome.reason(), Some(TeardownReason::UserRequest));

    running.hand_off();
    settle().await;
    assert!(!running.registered());
    assert_eq!(probe.calls(), 0);
    running.task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_dropped_finalizer_still_deregisters_and_the_watch_stands_down() {
    let probe = ScriptedProbe::new(&[true]);
    let harness = Harness::new(Metadata::Disabled);
    let registry = Arc::new(LaunchSessionRegistry::new());
    let (id, cancel_rx) = registry.register(SessionKind::Game, "steam-profile");
    let (handoff, handoff_rx) = oneshot::channel();
    let watch = SteamWatch {
        session_id: id,
        exe_name: "Game.exe".to_string(),
        registry: Arc::clone(&registry),
        outcome: WatchdogOutcome::new(),
        probe: probe.probe(),
    };
    let task = tokio::spawn(watch_steam_game(watch, cancel_rx, handoff_rx));

    let ctx = session_context(
        &harness,
        Arc::clone(&registry),
        id,
        &LaunchCase::new(SessionKind::Game, "steam_applaunch", String::new()),
    );
    let ctx = SessionContext {
        operation_id: None,
        deregister_handoff: Some(handoff),
        ..ctx
    };
    let (publisher, _events) = EventPublisher::channel(id);
    drop(SessionGuard::new(ctx, publisher));

    assert!(registry.snapshot().0.is_empty(), "Drop deregistered");
    task.await.unwrap();
    assert_eq!(probe.calls(), 0, "the watch never started probing");
}

/// The next `SessionExited` for `id` on the bus, as `(code, signal)`.
async fn exit_of(
    rx: &mut crate::events::EventReceiver,
    id: SessionId,
) -> (Option<i32>, Option<i32>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(AppEvent::SessionExited {
                session_id,
                code,
                signal,
                ..
            }) = rx.recv().await
            {
                if session_id == id {
                    return (code, signal);
                }
            }
        }
    })
    .await
    .expect("SessionExited within 5 s")
}

/// A Steam game guard on the service registry (bus observer attached) whose
/// deregister step may be handed off on the returned receiver.
fn steam_guard(harness: &Harness) -> (SessionGuard, SessionId, oneshot::Receiver<()>) {
    let registry = Arc::clone(harness.service.registry());
    let (id, _cancel) = registry.register(SessionKind::Game, "finalizer-profile");
    let (handoff, handoff_rx) = oneshot::channel();
    let ctx = session_context(
        harness,
        registry,
        id,
        &LaunchCase::new(SessionKind::Game, "steam_applaunch", String::new()),
    );
    let ctx = SessionContext {
        operation_id: None,
        deregister_handoff: Some(handoff),
        ..ctx
    };
    let (publisher, _events) = EventPublisher::channel(id);
    (SessionGuard::new(ctx, publisher), id, handoff_rx)
}

/// A helper that failed (non-zero code, a signal, or no status) never
/// started the game: the guard deregisters with the real status and never
/// hands off, so the watch stands down instead of probing.
#[tokio::test]
async fn a_failed_helper_deregisters_with_its_status_instead_of_handing_off() {
    use std::os::unix::process::ExitStatusExt;
    let harness = Harness::new(Metadata::Disabled);
    let mut rx = harness.events.subscribe();
    for (status, expected) in [
        (Some(exit_code(1)), (Some(1), None)),
        (Some(std::process::ExitStatus::from_raw(9)), (None, Some(9))),
        (None, (None, None)),
    ] {
        let (guard, id, mut handoff_rx) = steam_guard(&harness);
        guard.finish(status).await;
        assert_eq!(exit_of(&mut rx, id).await, expected, "{status:?}");
        assert_eq!(
            handoff_rx.try_recv(),
            Err(oneshot::error::TryRecvError::Closed),
            "{status:?}: no handoff; the dropped sender ends the watch"
        );
    }
    assert!(harness
        .service
        .running_sessions_snapshot()
        .sessions
        .is_empty());
}

/// A successful helper whose watch already finished (receiver gone) falls
/// back to deregistering with the real status.
#[tokio::test]
async fn a_refused_handoff_deregisters_with_the_real_status() {
    let harness = Harness::new(Metadata::Disabled);
    let mut rx = harness.events.subscribe();
    let (guard, id, handoff_rx) = steam_guard(&harness);
    drop(handoff_rx);
    guard.finish(Some(exit_code(0))).await;
    assert_eq!(exit_of(&mut rx, id).await, (Some(0), None));
    assert!(harness
        .service
        .running_sessions_snapshot()
        .sessions
        .is_empty());
}

/// End to end: a Steam helper that exits 1 ends the session right after
/// `Exited` with its code, and the watch never probes.
#[tokio::test]
async fn a_failed_steam_helper_ends_the_session_at_once_without_probing() {
    let mut harness = Harness::new(Metadata::Enabled);
    let calls = Arc::new(AtomicUsize::new(0));
    let probe_calls = Arc::clone(&calls);
    harness.service = harness
        .service
        .clone()
        .with_probe(Arc::new(move |_exe: String| {
            probe_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::ready(true))
        }));
    harness.spawner.push(Script::piped("exit 1"));
    let mut rx = harness.events.subscribe();

    let session = harness
        .service
        .start_game(harness.steam_request("steam-fail"))
        .await
        .expect("steam game starts");
    let id = session.id;
    Harness::collect(session).await;
    assert_eq!(exit_of(&mut rx, id).await, (Some(1), None));
    assert!(harness
        .service
        .running_sessions_snapshot()
        .sessions
        .is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0, "the watch never probed");
}

#[test]
fn only_steam_game_launches_get_the_watch() {
    let harness = Harness::new(Metadata::Disabled);
    let steam = harness.steam_request("watch-choice");
    let exe = steam.game_executable_name();
    assert!(!exe.is_empty());
    assert_eq!(
        steam_game_exe(&steam, SessionKind::Game, "steam_applaunch"),
        Some(exe)
    );
    assert_eq!(
        steam_game_exe(&steam, SessionKind::Trainer, "steam_applaunch"),
        None
    );
    assert_eq!(
        steam_game_exe(&steam, SessionKind::Game, "proton_run"),
        None
    );
    let trainer_only = crosshook_core::launch::LaunchRequest {
        launch_trainer_only: true,
        ..steam.clone()
    };
    assert_eq!(
        steam_game_exe(&trainer_only, SessionKind::Game, "steam_applaunch"),
        None
    );
}

/// End to end: the helper child exits, `launch-complete` arrives, yet the
/// session stays registered until the watch decides; a reset then ends it
/// with no exit status. The probe checks the executable name `run.rs`
/// derives from the request.
#[tokio::test]
async fn a_steam_game_session_outlives_its_helper_until_the_watch_ends_it() {
    let mut harness = Harness::new(Metadata::Enabled);
    let request = harness.steam_request("steam-watch");
    let expected_exe = request.game_executable_name();
    assert_eq!(expected_exe, "steam-watch.sh", "the game path's basename");
    let (probed_tx, mut probed_rx) = tokio::sync::mpsc::unbounded_channel();
    harness.service = harness
        .service
        .clone()
        .with_probe(Arc::new(move |exe: String| {
            let _ = probed_tx.send(exe);
            Box::pin(std::future::ready(true))
        }));
    let mut rx = harness.events.subscribe();

    let session = harness
        .service
        .start_game(request)
        .await
        .expect("steam game starts");
    let id = session.id;
    Harness::collect(session).await;
    let probed = tokio::time::timeout(Duration::from_secs(5), probed_rx.recv())
        .await
        .expect("the watch probes after the handoff")
        .expect("probe channel open");
    assert_eq!(probed, expected_exe, "the watch probes the game executable");

    let snapshot = harness.service.running_sessions_snapshot();
    assert_eq!(
        snapshot.sessions.len(),
        1,
        "helper exit does not end the session"
    );
    assert_eq!(snapshot.seq, 1, "no Exited published yet");

    assert_eq!(harness.service.reset_sessions("steam-watch"), 1);
    let exited = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(event @ AppEvent::SessionExited { .. }) = rx.recv().await {
                return event;
            }
        }
    })
    .await
    .expect("SessionExited after the reset");
    assert_eq!(
        exited,
        AppEvent::SessionExited {
            session_id: id,
            profile_name: Some("steam-watch".to_string()),
            kind: SessionKind::Game,
            code: None,
            signal: None,
            seq: 2,
        }
    );
}
