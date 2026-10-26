//! AC3 cascade tests (Task 4.1).
//!
//! A game's exit records `LinkedSessionExit` for its linked trainer only
//! (D1: it records the reason, it does not kill a non-gamescope trainer).
//! Unlinked trainers, sibling games and their trainers are untouched.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crosshook_core::launch::{session_profile_key_for_name, SessionId, SessionKind};
use tokio::time::timeout;

use super::super::{InjectionLevel, InjectionSource, LaunchEvent, LaunchSession};
use super::helpers::{Harness, Metadata, Script};

const TEARDOWN_MESSAGE: &str = "Trainer session teardown was recorded.";
const POLL_TIMEOUT: Duration = Duration::from_secs(5);

/// A session whose fake child blocks until its release file appears.
struct Blocked {
    session: LaunchSession,
    release: PathBuf,
    pid_file: PathBuf,
    prefix: Vec<LaunchEvent>,
}

impl Blocked {
    fn id(&self) -> SessionId {
        self.session.id
    }

    fn release(&self) {
        fs::write(&self.release, b"go").expect("write release file");
    }

    /// Takes the events queued so far (trainer pre-spawn injections) and
    /// keeps them as the head of the stream checked at the end.
    fn drain_checkpoint(&mut self) -> &[LaunchEvent] {
        self.prefix = self.session.events.drain_ready();
        &self.prefix
    }

    async fn finish(mut self) -> (Vec<LaunchEvent>, PathBuf) {
        let log_path = self.session.log_path.clone();
        let mut events = std::mem::take(&mut self.prefix);
        events.extend(Harness::collect(self.session).await);
        (events, log_path)
    }
}

fn blocked_script(release: &Path, pid_file: &Path, label: &str) -> Script {
    Script::piped(format!(
        "echo $$ > '{}'; while [ ! -f '{}' ]; do sleep 0.02; done; printf '{label}\\n'",
        pid_file.display(),
        release.display(),
    ))
}

async fn start_blocked(harness: &Harness, unique: &str, profile: &str, trainer: bool) -> Blocked {
    let release = harness.root.path().join(format!("release-{unique}"));
    let pid_file = harness.root.path().join(format!("pid-{unique}"));
    harness
        .spawner
        .push(blocked_script(&release, &pid_file, unique));

    // Unique exe name per launch (unique log slug), shared profile name so
    // sessions of one profile link to each other.
    let mut request = harness.proton_request(unique);
    request.profile_name = Some(profile.to_string());
    let session = if trainer {
        harness.service.start_trainer(request).await
    } else {
        harness.service.start_game(request).await
    }
    .expect("blocked launch starts");
    assert_eq!(
        session.kind,
        if trainer {
            SessionKind::Trainer
        } else {
            SessionKind::Game
        }
    );
    Blocked {
        session,
        release,
        pid_file,
        prefix: Vec::new(),
    }
}

async fn wait_for_pid(blocked: &Blocked) -> u32 {
    let pid_file = blocked.pid_file.clone();
    timeout(POLL_TIMEOUT, async move {
        loop {
            if let Ok(text) = fs::read_to_string(&pid_file) {
                if let Ok(pid) = text.trim().parse::<u32>() {
                    return pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("blocked child writes its pid file")
}

fn process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

fn is_registered(harness: &Harness, profile_key: &str, id: SessionId) -> bool {
    harness
        .service
        .registry()
        .sessions_for_profile(profile_key, None)
        .contains(&id)
}

async fn wait_deregistered(harness: &Harness, profile_key: &str, id: SessionId) {
    timeout(POLL_TIMEOUT, async {
        while is_registered(harness, profile_key, id) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("session deregisters after its Exited");
}

/// Gives a session's cancel-drain task its yield window so a reason sent
/// before deregistration is recorded before the child is released.
async fn yield_window() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

fn has_teardown_warning(events: &[LaunchEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            LaunchEvent::Injection { event, .. }
                if event.level == InjectionLevel::Warning
                    && event.source == InjectionSource::Runtime
                    && event.message == TEARDOWN_MESSAGE
        )
    })
}

fn exited_count(events: &[LaunchEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, LaunchEvent::Exited { .. }))
        .count()
}

fn assert_single_exited_last(events: &[LaunchEvent], id: SessionId) {
    assert!(
        events.iter().all(|event| event.session_id() == id),
        "every event carries its own session id: {events:?}"
    );
    assert_eq!(exited_count(events), 1, "exactly one Exited: {events:?}");
    assert!(
        matches!(events.last(), Some(LaunchEvent::Exited { .. })),
        "Exited is the last event: {events:?}"
    );
}

fn cleanup_log(path: &Path) {
    // Another test may race the /tmp log dir; never unwrap this removal.
    let _ = fs::remove_file(path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn game_exit_cascades_to_linked_trainer_only_and_siblings_survive() {
    let harness = Harness::new(Metadata::Enabled);
    let profile_a = "cascade-profile-a";
    let profile_b = "cascade-profile-b";
    let key_a = session_profile_key_for_name(Some(profile_a));
    let key_b = session_profile_key_for_name(Some(profile_b));

    // T0 starts before any game of profile A, so it stays unlinked.
    let mut t0 = start_blocked(&harness, "cascade-t0", profile_a, true).await;
    let g1 = start_blocked(&harness, "cascade-g1", profile_a, false).await;
    let t1 = start_blocked(&harness, "cascade-t1", profile_a, true).await;
    let mut g2 = start_blocked(&harness, "cascade-g2", profile_b, false).await;
    let mut t2 = start_blocked(&harness, "cascade-t2", profile_b, true).await;

    let (t0_id, g1_id, t1_id, g2_id, t2_id) = (t0.id(), g1.id(), t1.id(), g2.id(), t2.id());
    let log_paths: Vec<PathBuf> = [&t0, &g1, &t1, &g2, &t2]
        .iter()
        .map(|blocked| blocked.session.log_path.clone())
        .collect();

    let pids = [
        wait_for_pid(&t0).await,
        wait_for_pid(&g1).await,
        wait_for_pid(&t1).await,
        wait_for_pid(&g2).await,
        wait_for_pid(&t2).await,
    ];
    for (blocked, key) in [
        (&t0, &key_a),
        (&g1, &key_a),
        (&t1, &key_a),
        (&g2, &key_b),
        (&t2, &key_b),
    ] {
        assert!(is_registered(&harness, key, blocked.id()));
    }
    assert_eq!(
        harness.service.running_profiles(),
        vec![key_a.clone(), key_b.clone()]
    );

    // Release only G1 and wait for its terminal event.
    let g1_log = g1.session.log_path.clone();
    g1.release();
    let (g1_events, _) = g1.finish().await;
    assert_single_exited_last(&g1_events, g1_id);
    assert!(
        !g1_events
            .iter()
            .any(|event| matches!(event, LaunchEvent::Injection { .. })),
        "game sessions emit no injection rows"
    );

    // The cascade runs after `Exited` (finalizer step 10), so poll the
    // registry for deregistration instead of sleeping a fixed time.
    wait_deregistered(&harness, &key_a, g1_id).await;
    yield_window().await;

    assert!(!is_registered(&harness, &key_a, g1_id), "G1 deregistered");
    assert_eq!(
        harness.service.running_profiles(),
        vec![key_b.clone()],
        "only profile B still has a running game"
    );

    // Siblings survive: still registered, processes still alive (the
    // non-gamescope cascade records the reason; it does not kill, D1).
    for (blocked, key) in [(&t0, &key_a), (&t1, &key_a), (&g2, &key_b), (&t2, &key_b)] {
        assert!(
            is_registered(&harness, key, blocked.id()),
            "sibling session stays registered"
        );
    }
    for (label, pid) in [
        ("t0", pids[0]),
        ("t1", pids[2]),
        ("g2", pids[3]),
        ("t2", pids[4]),
    ] {
        assert!(process_alive(pid), "{label} process is still alive");
    }

    // None of the surviving sessions has been handed a terminal or cascade
    // event by G1's exit: only trainer pre-spawn injections may be queued.
    for (blocked, label) in [(&mut t0, "t0"), (&mut g2, "g2"), (&mut t2, "t2")] {
        let queued = blocked.drain_checkpoint();
        assert_eq!(
            exited_count(queued),
            0,
            "{label} received no Exited from G1's cascade: {queued:?}"
        );
        assert!(
            !has_teardown_warning(queued),
            "{label} received no teardown warning from G1's cascade: {queued:?}"
        );
    }

    // Releasing T1 proves the cascade landed: its drain recorded
    // `LinkedSessionExit`, so the wrap-up reports the recorded teardown.
    t1.release();
    let (t1_events, _) = t1.finish().await;
    assert_single_exited_last(&t1_events, t1_id);
    assert_eq!(
        t1_events
            .iter()
            .filter(|event| matches!(
                event,
                LaunchEvent::Injection { event, .. }
                    if event.level == InjectionLevel::Warning
                        && event.source == InjectionSource::Runtime
                        && event.message == TEARDOWN_MESSAGE
            ))
            .count(),
        1,
        "linked trainer records LinkedSessionExit as its teardown warning: {t1_events:?}"
    );

    // Unlinked and sibling sessions exit naturally, with no teardown
    // warning. Release each trainer before its game so a game's own
    // finalizer cannot cascade into a trainer that is still exiting.
    t0.release();
    let (t0_events, _) = t0.finish().await;
    assert_single_exited_last(&t0_events, t0_id);
    assert!(
        !has_teardown_warning(&t0_events),
        "unlinked trainer has no reason recorded: {t0_events:?}"
    );

    t2.release();
    let (t2_events, _) = t2.finish().await;
    assert_single_exited_last(&t2_events, t2_id);
    assert!(
        !has_teardown_warning(&t2_events),
        "sibling trainer has no reason recorded: {t2_events:?}"
    );

    g2.release();
    let (g2_events, _) = g2.finish().await;
    assert_single_exited_last(&g2_events, g2_id);
    assert!(
        !has_teardown_warning(&g2_events),
        "sibling game has no teardown warning: {g2_events:?}"
    );

    cleanup_log(&g1_log);
    for path in &log_paths {
        cleanup_log(path);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_sessions_signals_only_that_profile_once_each_without_cascading() {
    let harness = Harness::new(Metadata::Enabled);
    let profile_a = "cascade-reset-a";
    let profile_b = "cascade-reset-b";
    let key_a = session_profile_key_for_name(Some(profile_a));
    let key_b = session_profile_key_for_name(Some(profile_b));

    let ga = start_blocked(&harness, "cascade-reset-ga", profile_a, false).await;
    let ta = start_blocked(&harness, "cascade-reset-ta", profile_a, true).await;
    let mut gb = start_blocked(&harness, "cascade-reset-gb", profile_b, false).await;
    let mut tb = start_blocked(&harness, "cascade-reset-tb", profile_b, true).await;

    let (ga_id, ta_id, gb_id, tb_id) = (ga.id(), ta.id(), gb.id(), tb.id());
    let log_paths: Vec<PathBuf> = [&ga, &ta, &gb, &tb]
        .iter()
        .map(|blocked| blocked.session.log_path.clone())
        .collect();
    let gb_pid = wait_for_pid(&gb).await;
    let tb_pid = wait_for_pid(&tb).await;
    wait_for_pid(&ga).await;
    wait_for_pid(&ta).await;

    // Every live profile-A session is signalled once each; nothing else.
    assert_eq!(harness.service.reset_sessions(profile_a), 2);
    yield_window().await;

    // Reset signals; it does not deregister or kill anything.
    for (blocked, key) in [(&ga, &key_a), (&ta, &key_a), (&gb, &key_b), (&tb, &key_b)] {
        assert!(is_registered(&harness, key, blocked.id()));
    }
    assert!(process_alive(gb_pid), "profile B game still alive");
    assert!(process_alive(tb_pid), "profile B trainer still alive");

    // No profile-B session received a terminal or cascade event.
    for (blocked, label) in [(&mut gb, "gb"), (&mut tb, "tb")] {
        let queued = blocked.drain_checkpoint();
        assert_eq!(exited_count(queued), 0, "{label} got no Exited: {queued:?}");
        assert!(
            !has_teardown_warning(queued),
            "{label} got no teardown warning: {queued:?}"
        );
    }

    // The profile-A trainer was signalled directly by the reset: releasing
    // it while its game is still registered shows the recorded teardown
    // (a cascade could only have come from the game's exit).
    ta.release();
    let (ta_events, _) = ta.finish().await;
    assert_single_exited_last(&ta_events, ta_id);
    assert!(
        has_teardown_warning(&ta_events),
        "reset trainer records its UserRequest teardown: {ta_events:?}"
    );
    assert!(
        is_registered(&harness, &key_a, ga_id),
        "the reset trainer's game never exited, so this was no cascade"
    );

    ga.release();
    let (ga_events, _) = ga.finish().await;
    assert_single_exited_last(&ga_events, ga_id);

    // Profile B exits naturally with no reason recorded. Release the
    // trainer before its game for the same reason as the cascade test.
    tb.release();
    let (tb_events, _) = tb.finish().await;
    assert_single_exited_last(&tb_events, tb_id);
    assert!(
        !has_teardown_warning(&tb_events),
        "reset of profile A never signalled profile B: {tb_events:?}"
    );

    gb.release();
    let (gb_events, _) = gb.finish().await;
    assert_single_exited_last(&gb_events, gb_id);

    for path in &log_paths {
        cleanup_log(path);
    }
}
