use super::sessions::MAX_BUFFERED_EVENTS;
use super::{
    LibraryEffect as Effect, LibraryFilter, LibraryIntent as Intent, LibrarySort, LibraryViewModel,
    ProfileCard, RunningProfiles, SessionEvent, SessionKind,
};
use crate::testing::fixtures::{card, exited, running, started};
use crate::testing::Script;
use crate::{ServiceError, ViewModel};

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
    let mut tracker = RunningProfiles::default();
    tracker.apply_snapshot(
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
    let mut tracker = RunningProfiles::default();
    tracker.apply_snapshot(Vec::new(), 0);
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
    let mut tracker = RunningProfiles::default();
    tracker.apply_snapshot(vec![running("a", Some("alpha"), SessionKind::Game)], 5);
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
    assert!(!tracker.apply(started("c", Some("Gamma"), SessionKind::Game, 3)));
    assert!(!tracker.apply(started("b", Some("Beta"), SessionKind::Game, 5)));
    assert!(!tracker.is_synced());
    playing(&tracker, &[]);
    tracker.apply_snapshot(vec![running("a", Some("Alpha"), SessionKind::Game)], 4);
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
        .step(
            Intent::RunningSnapshot {
                sessions: vec![running("a", Some("alpha"), SessionKind::Game)],
                seq: 1,
            },
            [],
        )
        .step(game("b", "beta", 2), [])
        .step(
            Intent::Session(SessionEvent::Resync),
            [Effect::RequestRunningSessions],
        )
        .step(
            Intent::RunningSnapshot {
                sessions: vec![running("c", Some("gamma"), SessionKind::Game)],
                seq: 9,
            },
            [],
        )
        .assert_state(|vm| {
            assert_eq!(vm.currently_playing_count(), 1);
            assert!(vm.is_running("gamma"));
            assert!(!vm.is_running("alpha") && !vm.is_running("beta"));
        })
        .into_inner();
}

// 14
#[test]
fn duplicate_start_is_idempotent_and_unknown_exit_is_a_no_op() {
    let mut tracker = RunningProfiles::default();
    tracker.apply_snapshot(Vec::new(), 0);
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
        .step(
            Intent::RunningSnapshot {
                sessions: vec![running("a", Some("alpha"), SessionKind::Game)],
                seq: 1,
            },
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
    Script::new(loaded(plain(&["alpha"])))
        .step(Intent::SetFilter(LibraryFilter::CurrentlyRunning), [])
        .step(
            Intent::RunningSnapshot {
                sessions: Vec::new(),
                seq: 0,
            },
            [],
        )
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
    for seq in 1..=MAX_BUFFERED_EVENTS as u64 {
        let id = format!("s{seq}");
        assert!(vm.reduce(game(&id, "alpha", seq)).is_empty());
    }
    let overflow = MAX_BUFFERED_EVENTS as u64 + 1;
    Script::new(vm)
        .step(
            game("over", "beta", overflow),
            [Effect::RequestRunningSessions],
        )
        // The buffer starts again; the next event is buffered quietly.
        .step(game("next", "gamma", overflow + 1), [])
        // A snapshot older than the dropped events is stale.
        .step(
            Intent::RunningSnapshot {
                sessions: Vec::new(),
                seq: 3,
            },
            [],
        )
        .assert_state(|vm| assert_eq!(vm.currently_playing_count(), 0))
        // The requested snapshot covers the dropped events; the buffered tail applies.
        .step(
            Intent::RunningSnapshot {
                sessions: vec![running("over", Some("beta"), SessionKind::Game)],
                seq: overflow,
            },
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
        .step(
            Intent::ToggleFavorite("beta".into()),
            [Effect::SetFavorite {
                name: "beta".into(),
                favorite: true,
            }],
        )
        .assert_state(|vm| assert_eq!(names(vm), ["beta"]))
        .step(
            Intent::FavoriteSaved {
                name: "beta".into(),
                result: Err(unavailable()),
            },
            [],
        )
        .assert_state(|vm| {
            assert!(names(vm).is_empty());
            assert_eq!(vm.load_error(), Some(&unavailable()));
        })
        .step(Intent::ToggleFavorite("missing".into()), [])
        .into_inner();
}

#[test]
fn a_failed_save_restores_the_value_before_its_own_request() {
    // Two toggles in flight; the first save fails, the second succeeds.
    Script::new(loaded(plain(&["alpha"])))
        .step(
            Intent::ToggleFavorite("alpha".into()),
            [Effect::SetFavorite {
                name: "alpha".into(),
                favorite: true,
            }],
        )
        .step(
            Intent::ToggleFavorite("alpha".into()),
            [Effect::SetFavorite {
                name: "alpha".into(),
                favorite: false,
            }],
        )
        .step(
            Intent::FavoriteSaved {
                name: "alpha".into(),
                result: Err(unavailable()),
            },
            [],
        )
        .assert_state(|vm| assert!(!vm.visible().next().is_some_and(|c| c.is_favorite)))
        .step(
            Intent::FavoriteSaved {
                name: "alpha".into(),
                result: Ok(()),
            },
            [],
        )
        .assert_state(|vm| assert!(!vm.visible().next().is_some_and(|c| c.is_favorite)))
        .into_inner();
}
