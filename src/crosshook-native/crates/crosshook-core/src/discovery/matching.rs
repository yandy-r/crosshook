//! Token scoring, RSS title normalization, and advisory version matching for Phase B discovery.

use super::models::{VersionMatchResult, VersionMatchStatus};

/// Splits a string on non-alphanumeric characters, lowercases, and filters tokens < 2 chars.
pub(crate) fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.len() >= 2 {
                Some(token)
            } else {
                None
            }
        })
        .collect()
}

/// Counts how many `target_tokens` appear as substrings within `value`.
pub(crate) fn token_hits(value: &str, target_tokens: &[String]) -> usize {
    target_tokens
        .iter()
        .filter(|token| value.contains(token.as_str()))
        .count()
}

/// Strips the `" Trainer"` suffix and optional preceding version string from an RSS title.
///
/// Examples:
/// - `"Elden Ring Trainer"` -> `"Elden Ring"`
/// - `"Elden Ring v1.12 Trainer"` -> `"Elden Ring"`
/// - `"Elden Ring v1.12 +DLC Trainer"` -> `"Elden Ring"`
pub fn strip_trainer_suffix(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Find last occurrence of "Trainer" in the original string so byte indices
    // remain valid even when non-ASCII characters appear in the title.
    let Some(match_start) = last_ascii_case_insensitive_match(trimmed, "trainer") else {
        return trimmed.to_string();
    };

    // Only strip if "Trainer" is at word boundary (preceded by space or at start)
    if match_start > 0 && !trimmed.as_bytes()[match_start - 1].is_ascii_whitespace() {
        return trimmed.to_string();
    }

    let before_trainer = trimmed[..match_start].trim_end();
    if before_trainer.is_empty() {
        return trimmed.to_string();
    }

    // Strip optional version suffix like "v1.12", "v1.12 +DLC", "v1.12 (+N Trainer)" etc.
    strip_trailing_version(before_trainer)
}

/// Removes a trailing version pattern like `v1.12`, `v1.12 +DLC`, `v1.12 (+N)` from a name.
fn strip_trailing_version(name: &str) -> String {
    let trimmed = name.trim();

    // Strip trailing +DLC, +N, (+N Trainer) etc. first
    let cleaned = strip_trailing_plus_modifier(trimmed);

    // Now try to strip a trailing version like "v1.12" or "v1.12.3"
    let parts: Vec<&str> = cleaned.rsplitn(2, char::is_whitespace).collect();
    if parts.len() == 2 {
        let last_word = parts[0];
        let before = parts[1];
        if looks_like_version(last_word) && !before.is_empty() {
            return before.trim().to_string();
        }
    }

    cleaned.to_string()
}

/// Strips trailing `+DLC`, `+N`, `(+N ...)` modifiers.
fn strip_trailing_plus_modifier(name: &str) -> String {
    let trimmed = name.trim();

    // Handle parenthetical like "(+46 Trainer)" at end
    if let Some(paren_start) = trimmed.rfind('(') {
        let before_paren = trimmed[..paren_start].trim();
        if !before_paren.is_empty() {
            return before_paren.to_string();
        }
    }

    // Handle bare +DLC, +46 etc. at end
    if let Some(plus_pos) = trimmed.rfind('+') {
        if plus_pos > 0 && trimmed.as_bytes()[plus_pos - 1].is_ascii_whitespace() {
            let before_plus = trimmed[..plus_pos].trim();
            if !before_plus.is_empty() {
                return before_plus.to_string();
            }
        }
    }

    trimmed.to_string()
}

/// Checks whether a string looks like a version tag (e.g. `v1.12`, `v2`, `v1.0.3`).
fn looks_like_version(s: &str) -> bool {
    let s = s.to_lowercase();
    if !s.starts_with('v') {
        return false;
    }
    let after_v = &s[1..];
    if after_v.is_empty() {
        return false;
    }
    // Must start with a digit and contain only digits and dots
    after_v.chars().next().map_or(false, |c| c.is_ascii_digit())
        && after_v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn parse_version_segments(version: &str) -> Vec<&str> {
    version
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn is_true_prefix(a: &[&str], b: &[&str]) -> bool {
    a.len() < b.len() && b.starts_with(a)
}

fn last_ascii_case_insensitive_match(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }

    let mut last_match = None;
    let needle_len = needle.len();
    for (idx, _) in haystack.char_indices() {
        let Some(slice) = haystack[idx..].get(..needle_len) else {
            continue;
        };
        if slice.eq_ignore_ascii_case(needle) {
            last_match = Some(idx);
        }
    }

    last_match
}

/// Common stop words that appear in many game titles and cause false positive matches.
fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "of" | "the" | "and" | "in" | "on" | "at" | "to" | "for" | "an" | "or" | "by" | "dx"
    )
}

/// Scores how relevant an RSS result's game name is to a search query.
///
/// Returns `0.0..=1.0`. Results scoring `< 0.3` should be filtered by the caller.
pub fn score_fling_result(query: &str, rss_game_name: &str) -> f64 {
    let query_tokens: Vec<String> = tokenize(query)
        .into_iter()
        .filter(|t| !is_stop_word(t))
        .collect();
    if query_tokens.is_empty() {
        return 0.0;
    }

    let rss_lower = rss_game_name.to_lowercase();
    let hits = token_hits(&rss_lower, &query_tokens);
    hits as f64 / query_tokens.len() as f64
}

/// Advisory version comparison between a trainer's target game version and the installed version.
///
/// Always advisory, never blocking. Returns `Unknown` when either input is `None`.
pub fn match_trainer_version(
    trainer_game_version: Option<&str>,
    installed_human_game_ver: Option<&str>,
) -> VersionMatchResult {
    let (Some(trainer_ver), Some(installed_ver)) =
        (trainer_game_version, installed_human_game_ver)
    else {
        return VersionMatchResult {
            status: VersionMatchStatus::Unknown,
            trainer_game_version: trainer_game_version.map(String::from),
            installed_game_version: installed_human_game_ver.map(String::from),
            detail: Some("Version information is not available for comparison.".to_string()),
        };
    };

    let trainer_trimmed = trainer_ver.trim();
    let installed_trimmed = installed_ver.trim();

    if trainer_trimmed.is_empty() || installed_trimmed.is_empty() {
        return VersionMatchResult {
            status: VersionMatchStatus::Unknown,
            trainer_game_version: Some(trainer_trimmed.to_string()),
            installed_game_version: Some(installed_trimmed.to_string()),
            detail: Some("One or both version strings are empty.".to_string()),
        };
    }

    if trainer_trimmed == installed_trimmed {
        return VersionMatchResult {
            status: VersionMatchStatus::Exact,
            trainer_game_version: Some(trainer_trimmed.to_string()),
            installed_game_version: Some(installed_trimmed.to_string()),
            detail: Some("Trainer targets the exact installed game version.".to_string()),
        };
    }

    // Compatible only when one dot-separated version is a true prefix of the other
    // (e.g. 1.12 is a prefix of 1.12.3), not generic substring containment.
    let trainer_segments = parse_version_segments(trainer_trimmed);
    let installed_segments = parse_version_segments(installed_trimmed);
    if is_true_prefix(&trainer_segments, &installed_segments)
        || is_true_prefix(&installed_segments, &trainer_segments)
    {
        return VersionMatchResult {
            status: VersionMatchStatus::Compatible,
            trainer_game_version: Some(trainer_trimmed.to_string()),
            installed_game_version: Some(installed_trimmed.to_string()),
            detail: Some(format!(
                "Trainer targets {trainer_trimmed}, installed is {installed_trimmed}. Likely compatible."
            )),
        };
    }

    VersionMatchResult {
        status: VersionMatchStatus::Outdated,
        trainer_game_version: Some(trainer_trimmed.to_string()),
        installed_game_version: Some(installed_trimmed.to_string()),
        detail: Some(format!(
            "Trainer targets {trainer_trimmed} but installed version is {installed_trimmed}. May not be compatible."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // tokenize
    // -----------------------------------------------------------------------

    #[test]
    fn tokenize_splits_on_non_alphanumeric() {
        assert_eq!(tokenize("Elden Ring"), vec!["elden", "ring"]);
    }

    #[test]
    fn tokenize_lowercases() {
        assert_eq!(tokenize("DOOM Eternal"), vec!["doom", "eternal"]);
    }

    #[test]
    fn tokenize_filters_single_char() {
        assert_eq!(tokenize("A B cd ef"), vec!["cd", "ef"]);
    }

    #[test]
    fn tokenize_handles_empty_string() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn tokenize_handles_special_chars() {
        assert_eq!(tokenize("game-name_v2.0"), vec!["game", "name", "v2"]);
    }

    // -----------------------------------------------------------------------
    // strip_trainer_suffix
    // -----------------------------------------------------------------------

    #[test]
    fn strip_basic_trainer_suffix() {
        assert_eq!(strip_trainer_suffix("Elden Ring Trainer"), "Elden Ring");
    }

    #[test]
    fn strip_version_and_trainer_suffix() {
        assert_eq!(strip_trainer_suffix("Elden Ring v1.12 Trainer"), "Elden Ring");
    }

    #[test]
    fn strip_version_plus_dlc_and_trainer_suffix() {
        assert_eq!(
            strip_trainer_suffix("Elden Ring v1.12 +DLC Trainer"),
            "Elden Ring"
        );
    }

    #[test]
    fn strip_parenthetical_plus_trainer() {
        assert_eq!(
            strip_trainer_suffix("Elden Ring v1.12 (+46 Trainer)"),
            "Elden Ring"
        );
    }

    #[test]
    fn strip_no_suffix_unchanged() {
        assert_eq!(strip_trainer_suffix("Elden Ring"), "Elden Ring");
    }

    #[test]
    fn strip_empty_string() {
        assert_eq!(strip_trainer_suffix(""), "");
    }

    #[test]
    fn strip_only_trainer_word_unchanged() {
        assert_eq!(strip_trainer_suffix("Trainer"), "Trainer");
    }

    #[test]
    fn strip_case_insensitive() {
        assert_eq!(strip_trainer_suffix("Elden Ring TRAINER"), "Elden Ring");
        assert_eq!(strip_trainer_suffix("Elden Ring trainer"), "Elden Ring");
    }

    #[test]
    fn strip_handles_non_ascii_before_trainer() {
        assert_eq!(strip_trainer_suffix("Pokémon Trainer"), "Pokémon");
    }

    // -----------------------------------------------------------------------
    // score_fling_result
    // -----------------------------------------------------------------------

    #[test]
    fn score_exact_match_returns_one() {
        let score = score_fling_result("Elden Ring", "Elden Ring");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_partial_match_above_zero() {
        let score = score_fling_result("Elden Ring DLC", "Elden Ring");
        assert!(score > 0.0);
        assert!(score < 1.0);
    }

    #[test]
    fn score_unrelated_below_threshold() {
        let score = score_fling_result("Cyberpunk 2077", "Elden Ring");
        assert!(score < 0.3);
    }

    #[test]
    fn score_stop_words_do_not_inflate() {
        // "of" should be filtered as a stop word, so "ghost" and "tsushima"
        // are the real tokens — neither appears in "Atelier Ryza".
        let score = score_fling_result(
            "Ghost of Tsushima",
            "Atelier Ryza 3: Alchemist of the End & the Secret Key",
        );
        assert!(
            score < 0.3,
            "stop word 'of' should not cause a false match, got {score}"
        );
    }

    #[test]
    fn score_ghost_of_tsushima_matches_correctly() {
        let score = score_fling_result("Ghost of Tsushima", "Ghost of Tsushima");
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "exact match should score 1.0, got {score}"
        );
    }

    #[test]
    fn score_empty_query_returns_zero() {
        assert!((score_fling_result("", "Elden Ring")).abs() < f64::EPSILON);
    }

    #[test]
    fn score_empty_rss_name_returns_zero() {
        assert!((score_fling_result("Elden Ring", "")).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // match_trainer_version
    // -----------------------------------------------------------------------

    #[test]
    fn match_exact_version() {
        let result = match_trainer_version(Some("1.12"), Some("1.12"));
        assert_eq!(result.status, VersionMatchStatus::Exact);
    }

    #[test]
    fn match_compatible_version() {
        let result = match_trainer_version(Some("1.12"), Some("1.12.3"));
        assert_eq!(result.status, VersionMatchStatus::Compatible);
    }

    #[test]
    fn does_not_mark_substring_only_match_as_compatible() {
        let result = match_trainer_version(Some("1.1"), Some("1.12"));
        assert_eq!(result.status, VersionMatchStatus::Outdated);
    }

    #[test]
    fn match_outdated_version() {
        let result = match_trainer_version(Some("1.10"), Some("1.12"));
        assert_eq!(result.status, VersionMatchStatus::Outdated);
    }

    #[test]
    fn match_both_none_returns_unknown() {
        let result = match_trainer_version(None, None);
        assert_eq!(result.status, VersionMatchStatus::Unknown);
    }

    #[test]
    fn match_one_none_returns_unknown() {
        let result = match_trainer_version(Some("1.12"), None);
        assert_eq!(result.status, VersionMatchStatus::Unknown);

        let result = match_trainer_version(None, Some("1.12"));
        assert_eq!(result.status, VersionMatchStatus::Unknown);
    }

    #[test]
    fn match_empty_strings_returns_unknown() {
        let result = match_trainer_version(Some(""), Some("1.12"));
        assert_eq!(result.status, VersionMatchStatus::Unknown);
    }
}
