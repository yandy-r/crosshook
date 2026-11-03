//! Pure selectors for the library list: search, then filter, then sort.
//!
//! Parity oracle: React `useLibraryProfiles.ts` (search) and
//! `LibraryPage.tsx` (filter and sort). Search and filter are independent row
//! predicates; sort always runs last.

use std::cmp::Ordering;

use super::{LibraryFilter, LibrarySort, ProfileCard, RunningProfiles};

/// Builds the lowercase search keys [`visible_indices`] expects: two per card,
/// in card order, `[name_0, game_name_0, name_1, game_name_1, ...]`.
///
/// Compute them once per profile load, not per frame.
#[must_use]
pub fn search_keys(cards: &[ProfileCard]) -> Vec<String> {
    cards
        .iter()
        .flat_map(|card| [card.name.to_lowercase(), card.game_name.to_lowercase()])
        .collect()
}

/// Indices into `cards` of the visible cards, in display order.
///
/// `keys` must come from [`search_keys`] for the same `cards`.
///
/// - **Search**: `query` is trimmed with [`str::trim`] and lowercased; a card
///   matches when its lowercase `name` **or** `game_name` contains it (each
///   field on its own). An empty query matches every card. `str::trim` strips
///   U+0085 and keeps U+FEFF, the reverse of JS `String.prototype.trim`; this
///   only matters for queries made entirely of those characters.
/// - **Filter**: see [`LibraryFilter`]. `CurrentlyRunning` matches the exact,
///   case-sensitive `name` against `running`.
/// - **Sort**: [`LibrarySort::Recent`] keeps source order;
///   [`LibrarySort::Name`] uses [`name_order`].
///
/// # Panics
/// In debug builds, when `keys` does not hold two keys per card.
#[must_use]
pub fn visible_indices(
    cards: &[ProfileCard],
    keys: &[String],
    query: &str,
    filter: LibraryFilter,
    sort: LibrarySort,
    running: &RunningProfiles,
) -> Vec<usize> {
    debug_assert_eq!(keys.len(), cards.len() * 2, "two search keys per card");
    let query = query.trim().to_lowercase();
    let mut visible: Vec<usize> = cards
        .iter()
        .enumerate()
        .filter(|&(index, card)| {
            matches_query(keys, index, &query) && matches_filter(card, filter, running)
        })
        .map(|(index, _)| index)
        .collect();
    match sort {
        LibrarySort::Recent => {}
        LibrarySort::Name => visible.sort_by(|&a, &b| {
            compare_display(
                &cards[a],
                display_key_lower(cards, keys, a),
                &cards[b],
                display_key_lower(cards, keys, b),
            )
        }),
    }
    visible
}

/// Total order used by [`LibrarySort::Name`].
///
/// The display key is `game_name` when non-empty (a whitespace-only
/// `game_name` counts, as JS `||` does), otherwise `name`. Cards compare by:
/// 1. the lowercase display key;
/// 2. the display key with ASCII case swapped, byte-wise, so lowercase sorts
///    before uppercase (`"alpha"` before `"Alpha"`);
/// 3. `name`, which is unique, so the order is total and independent of the
///    input order.
///
/// This matches ICU `localeCompare` (the React oracle) for ASCII letters,
/// digits and spaces. Known divergences, deliberately not fixed (no ICU
/// dependency): ASCII punctuation such as `_` sorts after digits here and
/// before them in ICU; accented letters (`"éclair"`) sort after `z` here and
/// next to their base letter in ICU. Neither compares numbers numerically
/// (`"Game 10"` sorts before `"Game 2"`).
#[must_use]
pub fn name_order(a: &ProfileCard, b: &ProfileCard) -> Ordering {
    compare_display(
        a,
        &display_key(a).to_lowercase(),
        b,
        &display_key(b).to_lowercase(),
    )
}

/// Whether `card` passes `filter`.
fn matches_filter(card: &ProfileCard, filter: LibraryFilter, running: &RunningProfiles) -> bool {
    match filter {
        LibraryFilter::All => true,
        LibraryFilter::Favorites => card.is_favorite,
        LibraryFilter::Installed => !card.steam_app_id.is_empty() && card.steam_app_id != "0",
        LibraryFilter::CurrentlyRunning => running.contains(&card.name),
    }
}

fn matches_query(keys: &[String], index: usize, query: &str) -> bool {
    query.is_empty() || keys[2 * index].contains(query) || keys[2 * index + 1].contains(query)
}

fn display_key(card: &ProfileCard) -> &str {
    if card.game_name.is_empty() {
        &card.name
    } else {
        &card.game_name
    }
}

fn display_key_lower<'k>(cards: &[ProfileCard], keys: &'k [String], index: usize) -> &'k str {
    if cards[index].game_name.is_empty() {
        &keys[2 * index]
    } else {
        &keys[2 * index + 1]
    }
}

fn compare_display(a: &ProfileCard, a_lower: &str, b: &ProfileCard, b_lower: &str) -> Ordering {
    a_lower
        .cmp(b_lower)
        .then_with(|| {
            display_key(a)
                .bytes()
                .map(swap_ascii_case)
                .cmp(display_key(b).bytes().map(swap_ascii_case))
        })
        .then_with(|| a.name.cmp(&b.name))
}

fn swap_ascii_case(byte: u8) -> u8 {
    if byte.is_ascii_lowercase() {
        byte.to_ascii_uppercase()
    } else {
        byte.to_ascii_lowercase()
    }
}
