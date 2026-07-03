//! Pure ranking primitives for the cross-tap trainer catalog (Forgejo #27).
//!
//! No SQL here — both the metadata-DB path and the degraded filesystem path
//! rank through these functions, guaranteeing identical ordering semantics.

use std::cmp::{Ordering, Reverse};

use super::catalog::CatalogEntry;

/// Canonical compatibility band order, best first. The catalog facet order
/// and [`compatibility_rank`] both derive from this single array.
pub const COMPATIBILITY_BAND_ORDER: [&str; 5] =
    ["platinum", "working", "partial", "broken", "unknown"];

/// Match quality of a text query against a single name field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NameMatchTier {
    None = 0,
    Substring = 1,
    WordStart = 2,
    Prefix = 3,
}

/// Classify how `query` matches `name`.
///
/// `query` must be pre-trimmed + pre-lowercased and non-empty; `name` is
/// lowercased internally. Word starts are indices `i > 0` where a
/// non-alphanumeric character precedes an alphanumeric one.
pub fn name_match_tier(name: &str, query: &str) -> NameMatchTier {
    let name = name.to_lowercase();
    if name.starts_with(query) {
        return NameMatchTier::Prefix;
    }

    let mut previous: Option<char> = None;
    for (index, current) in name.char_indices() {
        if previous.is_some_and(|previous| !previous.is_alphanumeric())
            && current.is_alphanumeric()
            && name[index..].starts_with(query)
        {
            return NameMatchTier::WordStart;
        }
        previous = Some(current);
    }

    if name.contains(query) {
        return NameMatchTier::Substring;
    }

    NameMatchTier::None
}

/// Combined match score across game and trainer names:
/// game Prefix=6 > game WordStart=5 > game Substring=4 >
/// trainer Prefix=3 > trainer WordStart=2 > trainer Substring=1 > none=0.
pub fn catalog_match_score(game: Option<&str>, trainer: Option<&str>, query: &str) -> u8 {
    let game_component = match game.map_or(NameMatchTier::None, |name| name_match_tier(name, query))
    {
        NameMatchTier::Prefix => 6,
        NameMatchTier::WordStart => 5,
        NameMatchTier::Substring => 4,
        NameMatchTier::None => 0,
    };
    let trainer_component =
        match trainer.map_or(NameMatchTier::None, |name| name_match_tier(name, query)) {
            NameMatchTier::Prefix => 3,
            NameMatchTier::WordStart => 2,
            NameMatchTier::Substring => 1,
            NameMatchTier::None => 0,
        };
    game_component.max(trainer_component)
}

/// "platinum"=4, "working"=3, "partial"=2, "broken"=1, anything else (incl. `None`)=0.
/// Derived from position in [`COMPATIBILITY_BAND_ORDER`].
pub fn compatibility_rank(rating: Option<&str>) -> u8 {
    COMPATIBILITY_BAND_ORDER
        .iter()
        .position(|band| Some(*band) == rating)
        .map_or(0, |index| {
            (COMPATIBILITY_BAND_ORDER.len() - 1 - index) as u8
        })
}

/// Precomputed sort key equivalent to [`compare_catalog_rows`]: match score
/// DESC (0 in catalog mode), compatibility rank DESC, game name ASC
/// (case-insensitive, `None` last), manifest path ASC. Use with
/// `sort_by_cached_key` so score and case-folding run once per row instead
/// of once per comparison.
pub type CatalogSortKey = (Reverse<u8>, Reverse<u8>, bool, String, String);

pub fn catalog_sort_key(entry: &CatalogEntry, query: Option<&str>) -> CatalogSortKey {
    let score = query.map_or(0, |query| {
        catalog_match_score(
            entry.game_name.as_deref(),
            entry.trainer_name.as_deref(),
            query,
        )
    });
    let folded_name = entry.game_name.as_deref().map(str::to_lowercase);
    (
        Reverse(score),
        Reverse(compatibility_rank(entry.compatibility_rating.as_deref())),
        folded_name.is_none(),
        folded_name.unwrap_or_default(),
        entry.manifest_path.clone(),
    )
}

/// Total order over catalog entries. `query` is the pre-normalized
/// (trimmed + lowercased) text query; `None` means catalog mode —
/// mirroring Browse's historical `sortProfiles`.
pub fn compare_catalog_rows(a: &CatalogEntry, b: &CatalogEntry, query: Option<&str>) -> Ordering {
    catalog_sort_key(a, query).cmp(&catalog_sort_key(b, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        game_name: Option<&str>,
        trainer_name: Option<&str>,
        rating: Option<&str>,
        manifest_path: &str,
    ) -> CatalogEntry {
        CatalogEntry {
            id: None,
            tap_url: "https://example.com/tap.git".to_string(),
            tap_local_path: "/tmp/tap".to_string(),
            relative_path: manifest_path.to_string(),
            manifest_path: manifest_path.to_string(),
            game_name: game_name.map(str::to_string),
            game_version: None,
            trainer_name: trainer_name.map(str::to_string),
            trainer_version: None,
            proton_version: None,
            compatibility_rating: rating.map(str::to_string),
            author: None,
            description: None,
            platform_tags: None,
            trainer_loading_mode: None,
            schema_version: 1,
            sources: Vec::new(),
        }
    }

    #[test]
    fn prefix_outranks_word_start_outranks_substring() {
        assert_eq!(name_match_tier("Elden Ring", "eld"), NameMatchTier::Prefix);
        assert_eq!(
            name_match_tier("Shadow of the Erdtree", "erd"),
            NameMatchTier::WordStart
        );
        assert_eq!(
            name_match_tier("Elden Ring", "ing"),
            NameMatchTier::Substring
        );
        assert_eq!(name_match_tier("Elden Ring", "zzz"), NameMatchTier::None);

        assert!(NameMatchTier::Prefix > NameMatchTier::WordStart);
        assert!(NameMatchTier::WordStart > NameMatchTier::Substring);
        assert!(NameMatchTier::Substring > NameMatchTier::None);
    }

    #[test]
    fn game_name_match_outranks_trainer_name_match_at_equal_tier() {
        // Game substring (4) beats trainer prefix (3).
        assert_eq!(catalog_match_score(Some("Elden Ring"), None, "ing"), 4);
        assert_eq!(
            catalog_match_score(None, Some("Ingenious Trainer"), "ing"),
            3
        );
        // At equal tier, the game component is strictly higher.
        assert!(
            catalog_match_score(Some("Elden Ring"), None, "eld")
                > catalog_match_score(None, Some("Elden Trainer"), "eld")
        );
        // Both None contribute 0.
        assert_eq!(catalog_match_score(None, None, "eld"), 0);
    }

    #[test]
    fn matching_is_case_insensitive_and_unicode_aware() {
        assert_eq!(
            name_match_tier("ÉLDEN RING", "élden"),
            NameMatchTier::Prefix
        );
        assert_eq!(
            name_match_tier("Pokémon Snap", "poké"),
            NameMatchTier::Prefix
        );
        assert_eq!(
            name_match_tier("Best of Pokémon", "poké"),
            NameMatchTier::WordStart
        );
    }

    #[test]
    fn hyphen_and_colon_are_word_boundaries() {
        assert_eq!(
            name_match_tier("Elden Ring: Shadow", "sha"),
            NameMatchTier::WordStart
        );
        assert_eq!(
            name_match_tier("Half-Life", "life"),
            NameMatchTier::WordStart
        );
    }

    #[test]
    fn compatibility_breaks_ties_at_equal_match_score() {
        let platinum = entry(Some("Elden Ring"), None, Some("platinum"), "b.json");
        let working = entry(Some("Elden Ring II"), None, Some("working"), "a.json");
        assert_eq!(
            compare_catalog_rows(&platinum, &working, Some("elden")),
            std::cmp::Ordering::Less,
            "platinum must sort before working at equal match score"
        );
    }

    #[test]
    fn manifest_path_is_the_final_stable_tiebreak() {
        let first = entry(Some("Elden Ring"), None, Some("working"), "a.json");
        let second = entry(Some("Elden Ring"), None, Some("working"), "b.json");
        assert_eq!(
            compare_catalog_rows(&first, &second, Some("elden")),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_catalog_rows(&first, &second, None),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn catalog_mode_order_matches_browse_sort_profiles() {
        let mut rows = [
            entry(Some("Zeta"), None, Some("working"), "z.json"),
            entry(Some("alpha"), None, Some("working"), "a.json"),
            entry(Some("Middle"), None, Some("platinum"), "m.json"),
            entry(Some("Broken"), None, Some("broken"), "b.json"),
            entry(Some("Unrated"), None, None, "u.json"),
        ];
        rows.sort_by(|a, b| compare_catalog_rows(a, b, None));

        let names: Vec<&str> = rows.iter().filter_map(|r| r.game_name.as_deref()).collect();
        assert_eq!(
            names,
            ["Middle", "alpha", "Zeta", "Broken", "Unrated"],
            "rank DESC, then name ASC case-insensitive"
        );
    }

    #[test]
    fn none_game_names_sort_last() {
        let mut rows = [
            entry(None, Some("Nameless Trainer"), Some("working"), "n.json"),
            entry(Some("Alpha"), None, Some("working"), "a.json"),
        ];
        rows.sort_by(|a, b| compare_catalog_rows(a, b, None));
        assert_eq!(rows[0].game_name.as_deref(), Some("Alpha"));
        assert_eq!(rows[1].game_name, None);
    }
}
