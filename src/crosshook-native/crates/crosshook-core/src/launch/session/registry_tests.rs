//! Tests for [`super::LaunchSessionRegistry`], split out of `registry.rs`
//! to keep the production module under the repo's ~500-line soft cap.

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

#[test]
fn active_profile_keys_returns_sorted_deduped_profiles() {
    let registry = LaunchSessionRegistry::new();
    let (_b_game, _b_game_rx) = registry.register(SessionKind::Game, "profile-b");
    let (_a_game, _a_game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (_a_trainer, _a_trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (_b_trainer, _b_trainer_rx) = registry.register(SessionKind::Trainer, "profile-b");

    assert_eq!(
        registry.active_profile_keys(None),
        vec!["profile-a".to_string(), "profile-b".to_string()]
    );
}

#[test]
fn active_profile_keys_filters_to_games() {
    let registry = LaunchSessionRegistry::new();
    let (_trainer_only, _trainer_only_rx) = registry.register(SessionKind::Trainer, "trainer-only");
    let (_b_game, _b_game_rx) = registry.register(SessionKind::Game, "profile-b");
    let (_a_trainer, _a_trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
    let (_a_game, _a_game_rx) = registry.register(SessionKind::Game, "profile-a");
    let (_a_second_game, _a_second_game_rx) = registry.register(SessionKind::Game, "profile-a");

    assert_eq!(
        registry.active_profile_keys(Some(SessionKind::Game)),
        vec!["profile-a".to_string(), "profile-b".to_string()]
    );
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
