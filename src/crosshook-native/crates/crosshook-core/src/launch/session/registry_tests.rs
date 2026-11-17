//! Tests for [`super::LaunchSessionRegistry`].

use super::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn registry_is_send_and_sync() {
    assert_send_sync::<LaunchSessionRegistry>();
}

#[test]
fn register_and_deregister_round_trip() {
    let registry = LaunchSessionRegistry::new();
    let (id, _rx) = registry.register(SessionKind::Game, "profile-a");
    assert_eq!(registry.len(), 1);

    registry.deregister(id);
    assert_eq!(registry.len(), 0);

    // Idempotent.
    registry.deregister(id);
    assert_eq!(registry.len(), 0);
}

#[test]
fn link_to_parent_happy_path() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (trainer_id, _trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");

    registry
        .link_to_parent(trainer_id, game_id)
        .expect("link should succeed");
}

#[test]
fn link_to_parent_rejects_missing_parent() {
    let registry = LaunchSessionRegistry::new();
    let (trainer_id, _rx) = registry.register(SessionKind::Trainer, "profile-a");
    let phantom = SessionId::new();

    assert_eq!(
        registry.link_to_parent(trainer_id, phantom),
        Err(LinkError::ParentNotFound),
    );
}

#[test]
fn link_to_parent_rejects_missing_child() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _rx) = registry.register(SessionKind::Game, "profile-a");
    let phantom = SessionId::new();

    assert_eq!(
        registry.link_to_parent(phantom, game_id),
        Err(LinkError::ChildNotFound),
    );
}

#[test]
fn link_to_parent_rejects_cross_profile() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _g) = registry.register(SessionKind::Game, "profile-a");
    let (trainer_id, _t) = registry.register(SessionKind::Trainer, "profile-b");

    assert_eq!(
        registry.link_to_parent(trainer_id, game_id),
        Err(LinkError::Incompatible),
    );
}

#[test]
fn link_to_parent_rejects_trainer_as_parent() {
    let registry = LaunchSessionRegistry::new();
    let (p, _p_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (c, _c_rx) = registry.register(SessionKind::Trainer, "profile-a");

    assert_eq!(registry.link_to_parent(c, p), Err(LinkError::Incompatible),);
}

#[test]
fn link_to_parent_rejects_double_link() {
    let registry = LaunchSessionRegistry::new();
    let (g1, _g1_rx) = registry.register(SessionKind::Game, "profile-a");
    let (g2, _g2_rx) = registry.register(SessionKind::Game, "profile-a");
    let (t, _t_rx) = registry.register(SessionKind::Trainer, "profile-a");

    registry.link_to_parent(t, g1).expect("first link ok");
    assert_eq!(
        registry.link_to_parent(t, g2),
        Err(LinkError::AlreadyLinked),
    );
}

#[tokio::test]
async fn cancel_linked_children_reaches_only_linked_children() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (linked_trainer_id, mut linked_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (unlinked_trainer_id, mut unlinked_rx) =
        registry.register(SessionKind::Trainer, "profile-a");

    registry
        .link_to_parent(linked_trainer_id, game_id)
        .expect("link ok");

    let signalled = registry.cancel_linked_children(game_id, TeardownReason::LinkedSessionExit);
    assert_eq!(signalled, 1, "only the linked trainer should be signalled");

    let received = linked_rx.recv().await.expect("linked trainer gets signal");
    assert_eq!(received, TeardownReason::LinkedSessionExit);
    assert!(
        unlinked_rx.try_recv().is_err(),
        "unlinked trainer must not receive the cancel"
    );

    // Sanity — silence unused warnings.
    let _ = unlinked_trainer_id;
}

#[tokio::test]
async fn cancel_session_targets_exactly_one() {
    let registry = LaunchSessionRegistry::new();
    let (id, mut rx) = registry.register(SessionKind::Trainer, "profile-a");

    assert!(registry.cancel_session(id, TeardownReason::UserRequest));
    let received = rx.recv().await.expect("session receives cancel");
    assert_eq!(received, TeardownReason::UserRequest);

    registry.deregister(id);
    assert!(!registry.cancel_session(id, TeardownReason::UserRequest));
}

#[tokio::test]
async fn cancel_sessions_for_profile_cancels_all_kinds_for_key() {
    let registry = LaunchSessionRegistry::new();
    let (_game_id, mut game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (_trainer_id, mut trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (_other_id, mut other_rx) = registry.register(SessionKind::Game, "profile-b");

    let cancelled = registry.cancel_sessions_for_profile("profile-a", TeardownReason::UserRequest);
    assert_eq!(cancelled, 2, "game and trainer sessions are cancelled");

    assert_eq!(
        game_rx.recv().await.expect("game receives cancel"),
        TeardownReason::UserRequest
    );
    assert_eq!(
        trainer_rx.recv().await.expect("trainer receives cancel"),
        TeardownReason::UserRequest
    );
    assert!(
        other_rx.try_recv().is_err(),
        "other profile's session must not be cancelled"
    );
}

#[tokio::test]
async fn cancel_sessions_for_profile_signals_linked_trainer_exactly_once() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (trainer_id, mut trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
    registry
        .link_to_parent(trainer_id, game_id)
        .expect("link ok");

    let cancelled = registry.cancel_sessions_for_profile("profile-a", TeardownReason::UserRequest);
    assert_eq!(cancelled, 2);

    assert_eq!(
        trainer_rx.recv().await.expect("trainer receives cancel"),
        TeardownReason::UserRequest
    );
    assert!(
        trainer_rx.try_recv().is_err(),
        "linked trainer must not be double-cancelled via the parent cascade"
    );
}

#[test]
fn cancel_sessions_for_profile_returns_zero_for_unknown_key() {
    let registry = LaunchSessionRegistry::new();
    let (_id, _rx) = registry.register(SessionKind::Game, "profile-a");

    assert_eq!(
        registry.cancel_sessions_for_profile("missing-profile", TeardownReason::UserRequest),
        0
    );
}

#[test]
fn sessions_for_profile_filters_by_kind() {
    let registry = LaunchSessionRegistry::new();
    let (game_id, _g_rx) = registry.register(SessionKind::Game, "profile-a");
    let (trainer_id, _t_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (_other_game_id, _o_rx) = registry.register(SessionKind::Game, "profile-b");

    // Order-invariant asserts: a future test that adds a second matching
    // session (see sessions_for_profile_returns_most_recent_first) would
    // otherwise flake on HashMap iteration order.
    let games = registry.sessions_for_profile("profile-a", Some(SessionKind::Game));
    assert_eq!(games.len(), 1);
    assert!(games.contains(&game_id));

    let trainers = registry.sessions_for_profile("profile-a", Some(SessionKind::Trainer));
    assert_eq!(trainers.len(), 1);
    assert!(trainers.contains(&trainer_id));

    let all = registry.sessions_for_profile("profile-a", None);
    assert_eq!(all.len(), 2);
}

#[test]
fn sessions_for_profile_returns_most_recent_first() {
    let registry = LaunchSessionRegistry::new();
    let (first_game_id, _f_rx) = registry.register(SessionKind::Game, "profile-a");
    // Force a monotonic Instant tick so the second entry sorts strictly
    // after the first even on fast clocks.
    std::thread::sleep(std::time::Duration::from_millis(2));
    let (second_game_id, _s_rx) = registry.register(SessionKind::Game, "profile-a");

    let games = registry.sessions_for_profile("profile-a", Some(SessionKind::Game));
    assert_eq!(games.len(), 2);
    assert_eq!(
        games[0], second_game_id,
        "most-recently-registered game should be first"
    );
    assert_eq!(games[1], first_game_id);
}

/// Observer that records every change it sees, in call order.
fn recording_registry() -> (
    LaunchSessionRegistry,
    std::sync::Arc<std::sync::Mutex<Vec<SessionChange>>>,
) {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&seen);
    let registry = LaunchSessionRegistry::with_observer(std::sync::Arc::new(move |change| {
        sink.lock().unwrap().push(change.clone());
    }));
    (registry, seen)
}

#[test]
fn exited_is_emitted_exactly_once_and_only_on_actual_removal() {
    let (registry, seen) = recording_registry();
    let (id, _rx) = registry.register(SessionKind::Game, "profile-a");

    assert!(registry.deregister_with_exit(id, Some(3), None));
    assert!(!registry.deregister_with_exit(id, Some(4), None));
    registry.deregister(id);
    assert!(!registry.deregister_with_exit(SessionId::new(), None, Some(9)));

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "one Started, one Exited: {seen:?}");
    match &seen[1] {
        SessionChange::Exited {
            info, code, signal, ..
        } => {
            assert_eq!(info.session_id, id);
            assert_eq!((*code, *signal), (Some(3), None), "first removal wins");
        }
        other @ SessionChange::Started { .. } => panic!("expected Exited, got {other:?}"),
    }
    let (_, seq) = registry.snapshot();
    assert_eq!(seq, 2, "no-op removals do not bump seq");
}

#[test]
fn observer_sees_every_change_in_strict_seq_order() {
    let (registry, seen) = recording_registry();
    let (game, _game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (trainer, _trainer_rx, parent) = registry.register_and_link_to_parent_of_kind(
        SessionKind::Trainer,
        "profile-a",
        SessionKind::Game,
    );
    assert_eq!(parent, Some(game));
    // Linking and cancelling change no membership: no event, no seq bump.
    registry.cancel_linked_children(game, TeardownReason::LinkedSessionExit);
    registry.deregister(game);
    registry.deregister(trainer);

    let seen = seen.lock().unwrap();
    let seqs: Vec<u64> = seen.iter().map(SessionChange::seq).collect();
    assert_eq!(seqs, vec![1, 2, 3, 4]);
    assert!(matches!(&seen[1], SessionChange::Started { info, .. }
        if info.session_id == trainer && info.parent == Some(game)));
    assert!(matches!(&seen[2], SessionChange::Exited { info, .. } if info.session_id == game));
}

#[test]
fn snapshot_lists_sessions_oldest_first_with_the_current_seq() {
    let registry = LaunchSessionRegistry::new();
    assert_eq!(registry.snapshot(), (Vec::new(), 0));
    let (first, _rx1) = registry.register(SessionKind::Game, "profile-a");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let anonymous = super::super::ANONYMOUS_PROFILE_KEY;
    let (second, _rx2) = registry.register(SessionKind::Trainer, anonymous);
    registry.deregister(first);

    let (sessions, seq) = registry.snapshot();
    assert_eq!(seq, 3);
    assert_eq!(
        sessions,
        vec![SessionInfo {
            session_id: second,
            kind: SessionKind::Trainer,
            profile_key: anonymous.to_string(),
            parent: None,
        }]
    );
}

/// Four writers register/deregister while a reader snapshots concurrently.
/// Because `seq` is bumped and the observer called inside the mutation's
/// critical section, the observed stream is gap-free and ordered, and every
/// snapshot `(S, seq)` plus the changes with `seq > S` reproduces the final
/// state: no change is both in the snapshot and replayed, none is lost.
#[test]
fn concurrent_mutations_keep_snapshot_and_stream_consistent() {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Barrier};

    let (registry, seen) = recording_registry();
    let registry = Arc::new(registry);
    let barrier = Arc::new(Barrier::new(5));
    let snapshots = Arc::new(std::sync::Mutex::new(Vec::new()));

    let workers: Vec<_> = (0..4)
        .map(|worker| {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for round in 0..200 {
                    let (id, _rx) = registry.register(SessionKind::Game, format!("w{worker}"));
                    if round % 3 != 0 {
                        registry.deregister(id);
                    }
                }
            })
        })
        .collect();
    let reader = {
        let registry = Arc::clone(&registry);
        let barrier = Arc::clone(&barrier);
        let snapshots = Arc::clone(&snapshots);
        std::thread::spawn(move || {
            barrier.wait();
            for _ in 0..200 {
                snapshots.lock().unwrap().push(registry.snapshot());
            }
        })
    };
    for worker in workers {
        worker.join().unwrap();
    }
    reader.join().unwrap();

    let seen = seen.lock().unwrap();
    let seqs: Vec<u64> = seen.iter().map(SessionChange::seq).collect();
    assert_eq!(seqs, (1..=seen.len() as u64).collect::<Vec<_>>());
    let (final_sessions, final_seq) = registry.snapshot();
    assert_eq!(final_seq, seen.len() as u64);
    let expected: BTreeSet<String> = final_sessions
        .iter()
        .map(|info| info.session_id.to_string())
        .collect();

    for (sessions, seq) in snapshots.lock().unwrap().iter() {
        let mut state: BTreeSet<String> = sessions
            .iter()
            .map(|info| info.session_id.to_string())
            .collect();
        for change in seen.iter().filter(|change| change.seq() > *seq) {
            let id = change.info().session_id.to_string();
            match change {
                SessionChange::Started { .. } => assert!(state.insert(id), "double start"),
                SessionChange::Exited { .. } => assert!(state.remove(&id), "exit unknown"),
            }
        }
        assert_eq!(state, expected, "snapshot at seq {seq} + stream diverged");
    }
}

#[tokio::test]
async fn register_and_link_to_parent_of_kind_attaches_most_recent_parent() {
    let registry = LaunchSessionRegistry::new();
    let (_old_game_id, _old_rx) = registry.register(SessionKind::Game, "profile-a");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let (new_game_id, _new_rx) = registry.register(SessionKind::Game, "profile-a");

    let (trainer_id, mut trainer_rx, parent_id) = registry.register_and_link_to_parent_of_kind(
        SessionKind::Trainer,
        "profile-a",
        SessionKind::Game,
    );
    assert_eq!(parent_id, Some(new_game_id), "trainer links to newest game");

    let signalled = registry.cancel_linked_children(new_game_id, TeardownReason::LinkedSessionExit);
    assert_eq!(signalled, 1);
    let received = trainer_rx.recv().await.expect("trainer receives cascade");
    assert_eq!(received, TeardownReason::LinkedSessionExit);

    // Cleanup — silence lint on unused.
    let _ = trainer_id;
}

#[test]
fn register_and_link_to_parent_of_kind_returns_none_when_no_candidate() {
    let registry = LaunchSessionRegistry::new();
    let (trainer_id, _rx, parent_id) = registry.register_and_link_to_parent_of_kind(
        SessionKind::Trainer,
        "orphan-profile",
        SessionKind::Game,
    );
    assert_eq!(parent_id, None, "no game → no link");
    // Trainer is still registered — useful for the caller to deregister later.
    assert_eq!(registry.len(), 1);
    registry.deregister(trainer_id);
}

#[test]
fn register_and_link_to_parent_of_kind_rejects_illegal_pairings() {
    let registry = LaunchSessionRegistry::new();
    let (_existing_trainer, _rx1) = registry.register(SessionKind::Trainer, "profile-a");
    // Attempt a trainer → trainer link via the atomic API. The registry
    // must refuse the link even when a same-kind candidate exists, to
    // match link_to_parent's validation contract.
    let (_new_trainer, _rx2, parent_id) = registry.register_and_link_to_parent_of_kind(
        SessionKind::Trainer,
        "profile-a",
        SessionKind::Trainer,
    );
    assert_eq!(
        parent_id, None,
        "trainer → trainer pairing must not be linked"
    );
}
