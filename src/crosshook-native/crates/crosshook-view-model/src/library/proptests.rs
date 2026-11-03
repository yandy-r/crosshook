//! Property tests P1-P6 (research-practices.md §5.1).

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::select::{name_order, search_keys, visible_indices};
use super::{
    LibraryFilter, LibrarySort, ProfileCard, RunningProfiles, RunningSession, SessionId,
    SessionKind,
};
use crate::testing::fixtures::{exited, running, started, synced};

/// Small alphabet so searches, case ties and display-key ties all happen.
const NAME: &str = "[a-cA-C ]{0,4}";

fn arb_cards() -> impl Strategy<Value = Vec<ProfileCard>> {
    prop::collection::btree_map(
        "[a-cA-C]{1,4}",
        (
            NAME,
            any::<bool>(),
            prop_oneof![Just(String::new()), Just("0".to_owned()), "[0-9]{1,3}"],
        ),
        0..12,
    )
    .prop_map(|cards| {
        cards
            .into_iter()
            .map(
                |(name, (game_name, is_favorite, steam_app_id))| ProfileCard {
                    name,
                    game_name,
                    steam_app_id,
                    is_favorite,
                },
            )
            .collect()
    })
}

fn arb_filter() -> impl Strategy<Value = LibraryFilter> {
    prop_oneof![
        Just(LibraryFilter::All),
        Just(LibraryFilter::Favorites),
        Just(LibraryFilter::Installed),
        Just(LibraryFilter::CurrentlyRunning),
    ]
}

fn arb_sort() -> impl Strategy<Value = LibrarySort> {
    prop_oneof![Just(LibrarySort::Recent), Just(LibrarySort::Name)]
}

fn arb_kind() -> impl Strategy<Value = SessionKind> {
    prop_oneof![Just(SessionKind::Game), Just(SessionKind::Trainer)]
}

/// Sessions with unique ids `s0..`, profile names drawn from the card-name
/// alphabet so some match cards.
fn arb_sessions() -> impl Strategy<Value = Vec<RunningSession>> {
    prop::collection::vec((prop::option::of("[a-cA-C]{1,2}"), arb_kind()), 0..8).prop_map(
        |sessions| {
            sessions
                .into_iter()
                .enumerate()
                .map(|(index, (name, kind))| running(&format!("s{index}"), name.as_deref(), kind))
                .collect()
        },
    )
}

/// The running set as the spec defines it, computed from a session list.
fn named_games(sessions: &[RunningSession]) -> BTreeSet<String> {
    sessions
        .iter()
        .filter(|session| session.kind == SessionKind::Game)
        .filter_map(|session| session.profile_name.clone())
        .collect()
}

fn tracker_for(names: &[String]) -> RunningProfiles {
    let sessions = names
        .iter()
        .enumerate()
        .map(|(index, name)| running(&format!("r{index}"), Some(name), SessionKind::Game))
        .collect();
    synced(sessions, 0)
}

/// Independent per-card predicate: the spec, written without the selector.
fn expected_visible(
    card: &ProfileCard,
    query: &str,
    filter: LibraryFilter,
    running: &[String],
) -> bool {
    let q = query.trim().to_lowercase();
    let searched = q.is_empty()
        || card.name.to_lowercase().contains(&q)
        || card.game_name.to_lowercase().contains(&q);
    let filtered = match filter {
        LibraryFilter::All => true,
        LibraryFilter::Favorites => card.is_favorite,
        LibraryFilter::Installed => !matches!(card.steam_app_id.as_str(), "" | "0"),
        LibraryFilter::CurrentlyRunning => running.contains(&card.name),
    };
    searched && filtered
}

fn visible(
    cards: &[ProfileCard],
    query: &str,
    filter: LibraryFilter,
    sort: LibrarySort,
    running: &RunningProfiles,
) -> Vec<usize> {
    visible_indices(cards, &search_keys(cards), query, filter, sort, running)
}

proptest! {
    /// P1: soundness and completeness of search + filter.
    #[test]
    fn p1_visible_is_exactly_the_matching_cards(
        cards in arb_cards(),
        query in NAME,
        filter in arb_filter(),
        sort in arb_sort(),
        running in prop::collection::vec("[a-cA-C]{1,4}", 0..4),
    ) {
        let shown: BTreeSet<usize> =
            visible(&cards, &query, filter, sort, &tracker_for(&running)).into_iter().collect();
        let expected: BTreeSet<usize> = (0..cards.len())
            .filter(|&index| expected_visible(&cards[index], &query, filter, &running))
            .collect();
        prop_assert_eq!(shown, expected);
    }

    /// P2: `Recent` keeps source order; `Name` is a sorted permutation.
    #[test]
    fn p2_order_matches_the_sort(
        cards in arb_cards(),
        query in NAME,
        filter in arb_filter(),
    ) {
        let running = tracker_for(&[]);
        let recent = visible(&cards, &query, filter, LibrarySort::Recent, &running);
        prop_assert!(recent.windows(2).all(|pair| pair[0] < pair[1]));
        let by_name = visible(&cards, &query, filter, LibrarySort::Name, &running);
        prop_assert!(by_name
            .windows(2)
            .all(|pair| name_order(&cards[pair[0]], &cards[pair[1]]).is_lt()));
        let mut sorted = by_name.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, recent);
    }

    /// P3: extending the query never shows more cards.
    #[test]
    fn p3_longer_queries_narrow_the_list(
        cards in arb_cards(),
        query in NAME,
        suffix in NAME,
        filter in arb_filter(),
    ) {
        let running = tracker_for(&[]);
        let shorter: BTreeSet<usize> =
            visible(&cards, &query, filter, LibrarySort::Recent, &running).into_iter().collect();
        let longer = visible(&cards, &format!("{query}{suffix}"), filter, LibrarySort::Recent, &running);
        prop_assert!(longer.iter().all(|index| shorter.contains(index)));
    }

    /// P4: under `Name`, the visible names do not depend on input order.
    #[test]
    fn p4_name_sort_ignores_input_order(
        (cards, shuffled) in arb_cards().prop_flat_map(|cards| {
            let shuffled = Just(cards.clone()).prop_shuffle();
            (Just(cards), shuffled)
        }),
        query in NAME,
        filter in arb_filter(),
    ) {
        let running = tracker_for(&[]);
        let names = |cards: &[ProfileCard]| -> Vec<String> {
            visible(cards, &query, filter, LibrarySort::Name, &running)
                .into_iter()
                .map(|index| cards[index].name.clone())
                .collect()
        };
        prop_assert_eq!(names(&cards), names(&shuffled));
    }

    /// P5: events at or below the snapshot seq never change the running set.
    #[test]
    fn p5_stale_events_leave_the_snapshot_state(
        snapshot in arb_sessions(),
        seq in 0u64..50,
        events in prop::collection::vec(
            (any::<bool>(), "s[0-9]{1,2}", prop::option::of("[a-cA-C]{1,2}"), arb_kind(), any::<u64>()),
            0..16,
        ),
    ) {
        let mut tracker = synced(snapshot.clone(), seq);
        for (start, id, name, kind, raw) in events {
            let event_seq = raw % (seq + 1);
            let event = if start {
                started(&id, name.as_deref(), kind, event_seq)
            } else {
                exited(&id, event_seq)
            };
            prop_assert_eq!(tracker.apply(event), None);
        }
        let expected = named_games(&snapshot);
        prop_assert_eq!(tracker.currently_playing_count(), expected.len());
        prop_assert!(expected.iter().all(|name| tracker.contains(name)));
    }

    /// P6: the count equals the distinct named live game sessions of a
    /// reference model, and is bounded by the live game sessions.
    #[test]
    fn p6_count_matches_distinct_named_live_games(
        ops in prop::collection::vec(
            (any::<bool>(), 0usize..6, prop::option::of("[a-c]{1}"), arb_kind()),
            0..40,
        ),
    ) {
        let mut tracker = synced(Vec::new(), 0);
        let mut live: BTreeMap<SessionId, RunningSession> = BTreeMap::new();
        for (seq, (start, slot, name, kind)) in (1u64..).zip(ops) {
            let id = format!("s{slot}");
            if start {
                tracker.apply(started(&id, name.as_deref(), kind, seq));
                live.entry(SessionId::new(&id))
                    .or_insert_with(|| running(&id, name.as_deref(), kind));
            } else {
                tracker.apply(exited(&id, seq));
                live.remove(&SessionId::new(&id));
            }
            let sessions: Vec<RunningSession> = live.values().cloned().collect();
            let live_games = sessions.iter().filter(|s| s.kind == SessionKind::Game).count();
            let count = tracker.currently_playing_count();
            prop_assert!(count <= live_games);
            prop_assert_eq!(count, named_games(&sessions).len());
        }
    }
}
