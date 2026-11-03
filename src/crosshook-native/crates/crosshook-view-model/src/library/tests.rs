use super::sessions::MAX_BUFFERED_EVENTS;
use super::{
    LibraryEffect as Effect, LibraryFilter, LibraryIntent as Intent, LibrarySort, LibraryViewModel,
    ProfileCard, RunningProfiles, RunningSession, SessionEvent, SessionKind,
};
use crate::testing::fixtures::{card, exited, running, started, synced};
use crate::testing::Script;
use crate::{ServiceError, ServiceErrorKind, ViewModel};

fn snapshot(token: u64, sessions: Vec<RunningSession>, seq: u64) -> Intent {
    Intent::RunningSnapshot {
        token,
        sessions,
        seq,
    }
}

fn request(token: u64) -> Effect {
    Effect::RequestRunningSessions { token }
}

/// A library with `names` loaded and the first snapshot (token 1, empty, at
/// `seq` 0) applied.
fn started_library(names: &[&str]) -> Script<LibraryViewModel> {
    Script::new(loaded(plain(names)))
        .step(Intent::Start, [request(1)])
        .step(snapshot(1, Vec::new(), 0), [])
}

fn toggle(name: &str) -> Intent {
    Intent::ToggleFavorite(name.into())
}

fn set(name: &str, favorite: bool) -> Effect {
    Effect::SetFavorite {
        name: name.into(),
        favorite,
    }
}

fn saved(name: &str, favorite: bool, result: Result<(), ServiceError>) -> Intent {
    Intent::FavoriteSaved {
        name: name.into(),
        favorite,
        result,
    }
}

#[track_caller]
fn shows_favorite(vm: &LibraryViewModel, name: &str, expected: bool) {
    let card = vm
        .visible()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("{name} should be visible"));
    assert_eq!(card.is_favorite, expected, "{name} favorite flag");
}

fn names(vm: &LibraryViewModel) -> Vec<&str> {
    vm.visible_names().collect()
}

fn loaded(cards: Vec<ProfileCard>) -> LibraryViewModel {
    Script::new(LibraryViewModel::default())
        .step(Intent::ProfilesLoaded(Ok(cards)), [])
        .into_inner()
}

fn plain(names: &[&str]) -> Vec<ProfileCard> {
    names.iter().map(|name| card(name).build()).collect()
}

fn unavailable() -> ServiceError {
    ServiceError::new(crate::ServiceErrorKind::Unavailable, "store unavailable")
}

fn game(id: &str, name: &str, seq: u64) -> Intent {
    Intent::Session(started(id, Some(name), SessionKind::Game, seq))
}

fn playing(tracker: &RunningProfiles, names: &[&str]) {
    assert_eq!(tracker.currently_playing_count(), names.len(), "count");
    for name in names {
        assert!(tracker.contains(name), "{name} should be running");
    }
}

// 1
#[test]
fn search_trims_and_ignores_case_on_name() {
    let vm = Script::new(loaded(plain(&["alpha", "beta"])))
        .step(Intent::SetQuery("  ALP ".into()), [])
        .into_inner();
    assert_eq!(names(&vm), ["alpha"]);
}

// 2
#[test]
fn search_matches_game_name_when_name_does_not() {
    let vm = Script::new(loaded(vec![
        card("p1").game_name("Elden Ring").build(),
        card("p2").game_name("Hades").build(),
    ]))
    .step(Intent::SetQuery("ring".into()), [])
    .into_inner();
    assert_eq!(names(&vm), ["p1"]);
}

// 3
#[test]
fn empty_or_whitespace_query_keeps_every_card_in_source_order() {
    let source = ["zeta", "alpha", "mid"];
    Script::new(loaded(plain(&source)))
        .assert_state(|vm| assert_eq!(names(vm), source))
        .step(Intent::SetQuery(" \t ".into()), [])
        .assert_state(|vm| assert_eq!(names(vm), source))
        .into_inner();
}

// 4
#[test]
fn favorites_filter_keeps_only_favorites() {
    let vm = Script::new(loaded(vec![
        card("a").favorite().build(),
        card("b").build(),
        card("c").favorite().build(),
    ]))
    .step(Intent::SetFilter(LibraryFilter::Favorites), [])
    .into_inner();
    assert_eq!(names(&vm), ["a", "c"]);
}

// 5
#[test]
fn installed_filter_excludes_empty_and_zero_app_ids() {
    let vm = Script::new(loaded(vec![
        card("empty").build(),
        card("zero").steam_app_id("0").build(),
        card("real").steam_app_id("123").build(),
    ]))
    .step(Intent::SetFilter(LibraryFilter::Installed), [])
    .into_inner();
    assert_eq!(names(&vm), ["real"]);
}

// 6
#[test]
fn name_sort_uses_game_name_then_name_with_a_deterministic_tie_break() {
    let vm = Script::new(loaded(vec![
        card("z-profile").game_name("Beta").build(),
        card("charlie").build(),
        card("y-profile").game_name("alpha").build(),
        card("x-profile").game_name("Alpha").build(),
        card("b-dup").game_name("Delta").build(),
        card("a-dup").game_name("Delta").build(),
    ]))
    .step(Intent::SetSort(LibrarySort::Name), [])
    .into_inner();
    // Lowercase before uppercase on a case-only tie; equal keys by `name`.
    assert_eq!(
        names(&vm),
        [
            "y-profile",
            "x-profile",
            "z-profile",
            "charlie",
            "a-dup",
            "b-dup"
        ]
    );
}

// 7
#[test]
fn search_filter_and_sort_compose() {
    Script::new(loaded(vec![
        card("gamma").game_name("Zelda").favorite().build(),
        card("alpha").build(),
        card("alpine").game_name("Celeste").favorite().build(),
        card("beta").game_name("Alpaca").build(),
    ]))
    .step(Intent::SetQuery("al".into()), [])
    .assert_state(|vm| assert_eq!(names(vm), ["alpha", "alpine", "beta"]))
    .step(Intent::SetFilter(LibraryFilter::Favorites), [])
    .assert_state(|vm| assert_eq!(names(vm), ["alpine"]))
    .step(Intent::SetQuery(String::new()), [])
    .assert_state(|vm| assert_eq!(names(vm), ["gamma", "alpine"]))
    .step(Intent::SetSort(LibrarySort::Name), [])
    .assert_state(|vm| assert_eq!(names(vm), ["alpine", "gamma"]))
    .into_inner();
}

// 8
#[test]
fn refresh_loads_and_a_failed_load_keeps_the_list_until_a_later_success() {
    Script::new(LibraryViewModel::default())
        .step(Intent::Refresh, [Effect::LoadProfiles])
        .step(Intent::ProfilesLoaded(Ok(plain(&["alpha"]))), [])
        .step(Intent::ProfilesLoaded(Err(unavailable())), [])
        .assert_state(|vm| {
            assert_eq!(names(vm), ["alpha"]);
            assert_eq!(vm.load_error(), Some(&unavailable()));
        })
        .step(Intent::Refresh, [Effect::LoadProfiles])
        .step(Intent::ProfilesLoaded(Ok(plain(&["beta"]))), [])
        .assert_state(|vm| {
            assert_eq!(names(vm), ["beta"]);
            assert_eq!(vm.load_error(), None);
        })
        .into_inner();
}

// 9
#[test]
fn only_named_game_sessions_count_as_playing() {
    let tracker = synced(
        vec![
            running("t", Some("alpha"), SessionKind::Trainer),
            running("anon", None, SessionKind::Game),
            running("g", Some("beta"), SessionKind::Game),
        ],
        1,
    );
    playing(&tracker, &["beta"]);
    assert!(!tracker.contains("alpha"));
}

// 10
#[test]
fn two_game_sessions_of_one_profile_count_once_until_both_exit() {
    let mut tracker = synced(Vec::new(), 0);
    tracker.apply(started("g1", Some("alpha"), SessionKind::Game, 1));
    tracker.apply(started("g2", Some("alpha"), SessionKind::Game, 2));
    playing(&tracker, &["alpha"]);
    tracker.apply(exited("g1", 3));
    playing(&tracker, &["alpha"]);
    tracker.apply(exited("g2", 4));
    playing(&tracker, &[]);
}

// 11
#[test]
fn events_at_or_below_the_snapshot_seq_are_ignored() {
    let mut tracker = synced(vec![running("a", Some("alpha"), SessionKind::Game)], 5);
    tracker.apply(started("b", Some("beta"), SessionKind::Game, 4));
    tracker.apply(started("c", Some("gamma"), SessionKind::Game, 5));
    tracker.apply(exited("a", 5));
    playing(&tracker, &["alpha"]);
    tracker.apply(started("d", Some("delta"), SessionKind::Game, 6));
    playing(&tracker, &["alpha", "delta"]);
}

// 12 (mirrors runningSessionsStore.test.ts:102)
#[test]
fn events_before_the_first_snapshot_are_buffered_and_filtered_by_seq() {
    let mut tracker = RunningProfiles::default();
    assert_eq!(
        tracker.apply(started("c", Some("Gamma"), SessionKind::Game, 3)),
        None
    );
    let token = tracker.request_snapshot();
    assert_eq!(
        tracker.apply(started("b", Some("Beta"), SessionKind::Game, 5)),
        None
    );
    assert!(!tracker.is_synced());
    playing(&tracker, &[]);
    assert!(tracker.apply_snapshot(
        token,
        vec![running("a", Some("Alpha"), SessionKind::Game)],
        4
    ));
    assert!(tracker.is_synced());
    playing(&tracker, &["Alpha", "Beta"]);
    tracker.apply(started("c", Some("Gamma"), SessionKind::Game, 4));
    tracker.apply(exited("a", 6));
    playing(&tracker, &["Beta"]);
}

// 13
#[test]
fn resync_requests_a_snapshot_that_replaces_the_state() {
    Script::new(LibraryViewModel::default())
        .step(Intent::Start, [request(1)])
        .step(
            snapshot(1, vec![running("a", Some("alpha"), SessionKind::Game)], 1),
            [],
        )
        .step(game("b", "beta", 2), [])
        .step(Intent::Session(SessionEvent::Resync), [request(2)])
        .step(
            snapshot(2, vec![running("c", Some("gamma"), SessionKind::Game)], 9),
            [],
        )
        .assert_state(|vm| {
            assert_eq!(vm.currently_playing_count(), 1);
            assert!(vm.is_running("gamma"));
            assert!(!vm.is_running("alpha") && !vm.is_running("beta"));
        })
        .into_inner();
}

#[test]
fn only_the_reply_to_the_latest_request_applies() {
    // Two resyncs in flight; the superseded reply (seq 10) must not make the
    // latest one (seq 11) look stale after a live event at seq 12.
    Script::new(LibraryViewModel::default())
        .step(Intent::Start, [request(1)])
        .step(snapshot(1, Vec::new(), 0), [])
        .step(Intent::Session(SessionEvent::Resync), [request(2)])
        .step(Intent::Session(SessionEvent::Resync), [request(3)])
        .step(
            snapshot(2, vec![running("old", Some("old"), SessionKind::Game)], 10),
            [],
        )
        .assert_state(|vm| {
            assert!(!vm.running.is_synced(), "superseded reply must not sync");
            assert_eq!(vm.currently_playing_count(), 0);
        })
        .step(game("live", "live", 12), [])
        .step(
            snapshot(
                3,
                vec![running("lost", Some("lost"), SessionKind::Game)],
                11,
            ),
            [],
        )
        .assert_state(|vm| {
            assert!(vm.is_running("lost") && vm.is_running("live"));
            assert!(!vm.is_running("old"));
            assert_eq!(vm.currently_playing_count(), 2);
        })
        // A late reply to an already answered request changes nothing.
        .step(snapshot(3, Vec::new(), 20), [])
        .step(snapshot(2, Vec::new(), 20), [])
        .assert_state(|vm| assert_eq!(vm.currently_playing_count(), 2))
        .into_inner();
}

#[test]
fn a_reply_without_an_outstanding_request_is_ignored() {
    Script::new(LibraryViewModel::default())
        // Before `Start`, nothing was requested; the event stays buffered.
        .step(game("b", "beta", 5), [])
        .step(
            snapshot(1, vec![running("a", Some("alpha"), SessionKind::Game)], 1),
            [],
        )
        .assert_state(|vm| assert_eq!(vm.currently_playing_count(), 0))
        .step(Intent::Start, [request(1)])
        .step(
            snapshot(1, vec![running("a", Some("alpha"), SessionKind::Game)], 4),
            [],
        )
        .assert_state(|vm| assert!(vm.is_running("alpha") && vm.is_running("beta")))
        .into_inner();
}

// 14
#[test]
fn duplicate_start_is_idempotent_and_unknown_exit_is_a_no_op() {
    let mut tracker = synced(Vec::new(), 0);
    tracker.apply(started("g", Some("alpha"), SessionKind::Game, 1));
    tracker.apply(started("g", Some("alpha"), SessionKind::Game, 2));
    tracker.apply(exited("ghost", 3));
    playing(&tracker, &["alpha"]);
    // One exit removes the session the duplicate start did not add twice.
    tracker.apply(exited("g", 4));
    playing(&tracker, &[]);
}

// 15
#[test]
fn running_filter_follows_session_events_without_a_reload() {
    Script::new(loaded(plain(&["alpha", "beta", "gamma"])))
        .step(Intent::SetFilter(LibraryFilter::CurrentlyRunning), [])
        .step(Intent::Start, [request(1)])
        .step(
            snapshot(1, vec![running("a", Some("alpha"), SessionKind::Game)], 1),
            [],
        )
        .assert_state(|vm| assert_eq!(names(vm), ["alpha"]))
        .step(game("g", "gamma", 2), [])
        .assert_state(|vm| assert_eq!(names(vm), ["alpha", "gamma"]))
        .step(Intent::Session(exited("a", 3)), [])
        .assert_state(|vm| assert_eq!(names(vm), ["gamma"]))
        .into_inner();
}

#[test]
fn running_profile_missing_from_the_library_counts_but_is_not_listed() {
    started_library(&["alpha"])
        .step(Intent::SetFilter(LibraryFilter::CurrentlyRunning), [])
        .step(game("r", "renamed", 1), [])
        .assert_state(|vm| {
            assert_eq!(vm.currently_playing_count(), 1);
            assert!(names(vm).is_empty());
        })
        .into_inner();
}

#[test]
fn buffer_overflow_drops_events_and_requests_one_snapshot() {
    let mut vm = LibraryViewModel::default();
    assert_eq!(vm.reduce(Intent::Start), [request(1)]);
    for seq in 1..=MAX_BUFFERED_EVENTS as u64 {
        let id = format!("s{seq}");
        assert!(vm.reduce(game(&id, "alpha", seq)).is_empty());
    }
    let overflow = MAX_BUFFERED_EVENTS as u64 + 1;
    Script::new(vm)
        .step(game("over", "beta", overflow), [request(2)])
        // The buffer starts again; the next event is buffered quietly.
        .step(game("next", "gamma", overflow + 1), [])
        // The reply to the superseded request may predate the dropped events.
        .step(
            snapshot(1, vec![running("s1", Some("alpha"), SessionKind::Game)], 0),
            [],
        )
        .assert_state(|vm| {
            assert!(!vm.running.is_synced());
            assert_eq!(vm.currently_playing_count(), 0);
        })
        // The new request covers the dropped events; the buffered tail applies.
        .step(
            snapshot(
                2,
                vec![running("over", Some("beta"), SessionKind::Game)],
                overflow,
            ),
            [],
        )
        .assert_state(|vm| {
            assert_eq!(vm.currently_playing_count(), 2);
            assert!(vm.is_running("beta") && vm.is_running("gamma"));
            assert!(!vm.is_running("alpha"));
        })
        .into_inner();
}

#[test]
fn toggle_favorite_is_optimistic_and_a_failed_save_reverts_it() {
    Script::new(loaded(plain(&["alpha", "beta"])))
        .step(Intent::SetFilter(LibraryFilter::Favorites), [])
        .step(toggle("beta"), [set("beta", true)])
        .assert_state(|vm| assert_eq!(names(vm), ["beta"]))
        .step(saved("beta", true, Err(unavailable())), [])
        .assert_state(|vm| {
            assert!(names(vm).is_empty());
            assert_eq!(vm.load_error(), Some(&unavailable()));
        })
        .step(toggle("missing"), [])
        .into_inner();
}

#[test]
fn when_every_save_fails_the_confirmed_value_returns() {
    Script::new(loaded(plain(&["alpha"])))
        .step(toggle("alpha"), [set("alpha", true)])
        .step(toggle("alpha"), [set("alpha", false)])
        .step(toggle("alpha"), [set("alpha", true)])
        .step(saved("alpha", true, Err(unavailable())), [])
        // Saves still in flight: the latest request stays on screen.
        .assert_state(|vm| shows_favorite(vm, "alpha", true))
        .step(saved("alpha", false, Err(unavailable())), [])
        .assert_state(|vm| shows_favorite(vm, "alpha", true))
        .step(saved("alpha", true, Err(unavailable())), [])
        .assert_state(|vm| shows_favorite(vm, "alpha", false))
        .into_inner();
}

#[test]
fn a_failed_first_save_does_not_undo_later_successes() {
    Script::new(loaded(plain(&["alpha"])))
        .step(toggle("alpha"), [set("alpha", true)])
        .step(toggle("alpha"), [set("alpha", false)])
        .step(toggle("alpha"), [set("alpha", true)])
        .step(saved("alpha", true, Err(unavailable())), [])
        .assert_state(|vm| shows_favorite(vm, "alpha", true))
        .step(saved("alpha", false, Ok(())), [])
        .assert_state(|vm| shows_favorite(vm, "alpha", true))
        .step(saved("alpha", true, Ok(())), [])
        .assert_state(|vm| {
            shows_favorite(vm, "alpha", true);
            assert_eq!(vm.load_error(), Some(&unavailable()));
        })
        .into_inner();
}

#[test]
fn out_of_order_replies_settle_on_the_last_confirmed_value() {
    // The second save answers first and succeeds; the first then fails.
    Script::new(loaded(plain(&["alpha", "beta"])))
        .step(toggle("alpha"), [set("alpha", true)])
        .step(toggle("alpha"), [set("alpha", false)])
        .step(saved("alpha", false, Ok(())), [])
        .step(saved("alpha", true, Err(unavailable())), [])
        .assert_state(|vm| shows_favorite(vm, "alpha", false))
        // The second save fails first; the first then succeeds.
        .step(toggle("beta"), [set("beta", true)])
        .step(toggle("beta"), [set("beta", false)])
        .step(saved("beta", false, Err(unavailable())), [])
        .assert_state(|vm| shows_favorite(vm, "beta", false))
        .step(saved("beta", true, Ok(())), [])
        .assert_state(|vm| shows_favorite(vm, "beta", true))
        .into_inner();
}

#[test]
fn a_reload_during_a_save_keeps_the_pending_value() {
    Script::new(loaded(plain(&["alpha", "beta"])))
        .step(toggle("alpha"), [set("alpha", true)])
        // The reload was read before the save landed: stale `alpha`, and a
        // `beta` change made elsewhere.
        .step(
            Intent::ProfilesLoaded(Ok(vec![
                card("alpha").build(),
                card("beta").favorite().build(),
            ])),
            [],
        )
        .assert_state(|vm| {
            shows_favorite(vm, "alpha", true);
            shows_favorite(vm, "beta", true);
        })
        .step(saved("alpha", true, Ok(())), [])
        .assert_state(|vm| shows_favorite(vm, "alpha", true))
        // Nothing pending: a reload is taken as is.
        .step(
            Intent::ProfilesLoaded(Ok(vec![card("alpha").build(), card("beta").build()])),
            [],
        )
        .assert_state(|vm| {
            shows_favorite(vm, "alpha", false);
            shows_favorite(vm, "beta", false);
        })
        .into_inner();
}

#[test]
fn a_reload_during_a_save_confirms_the_loaded_value() {
    Script::new(loaded(plain(&["alpha"])))
        .step(toggle("alpha"), [set("alpha", true)])
        // The store already holds `true` (written elsewhere, or by this save
        // before its reply was lost).
        .step(
            Intent::ProfilesLoaded(Ok(vec![card("alpha").favorite().build()])),
            [],
        )
        .step(
            saved(
                "alpha",
                true,
                Err(ServiceError::new(ServiceErrorKind::Io, "disk full")),
            ),
            [],
        )
        .assert_state(|vm| shows_favorite(vm, "alpha", true))
        .into_inner();
}
