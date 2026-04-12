use crate::protonup::{ProtonUpMatchStatus, ProtonUpSuggestion};
use crate::steam::proton::normalize_alias;
use crate::steam::ProtonInstall;

/// Compare a community-recommended Proton version against installed runtimes.
///
/// Returns an advisory suggestion — never a launch-blocking decision.
pub fn match_community_version(
    community_version: &str,
    installed: &[ProtonInstall],
) -> ProtonUpSuggestion {
    let trimmed = community_version.trim();

    if trimmed.is_empty() {
        return ProtonUpSuggestion {
            status: ProtonUpMatchStatus::Unknown,
            community_version: None,
            matched_install_name: None,
            recommended_version: None,
        };
    }

    // Exact match: case-insensitive alias comparison.
    let exact_match = installed.iter().find(|install| {
        install
            .aliases
            .iter()
            .any(|alias: &String| alias.eq_ignore_ascii_case(trimmed))
    });

    if let Some(install) = exact_match {
        return ProtonUpSuggestion {
            status: ProtonUpMatchStatus::Matched,
            community_version: Some(trimmed.to_string()),
            matched_install_name: Some(install.name.clone()),
            recommended_version: None,
        };
    }

    // Normalized match: strip non-alphanumeric, lowercase.
    if let Some(normalized_community) = normalize_alias(trimmed) {
        let normalized_match = installed
            .iter()
            .find(|install| install.normalized_aliases.contains(&normalized_community));

        if let Some(install) = normalized_match {
            return ProtonUpSuggestion {
                status: ProtonUpMatchStatus::Matched,
                community_version: Some(trimmed.to_string()),
                matched_install_name: Some(install.name.clone()),
                recommended_version: None,
            };
        }
    }

    // No match found.
    ProtonUpSuggestion {
        status: ProtonUpMatchStatus::Missing,
        community_version: Some(trimmed.to_string()),
        matched_install_name: None,
        recommended_version: Some(trimmed.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn test_install(name: &str, aliases: &[&str]) -> ProtonInstall {
        let aliases_vec: Vec<String> = aliases
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let normalized = aliases_vec
            .iter()
            .filter_map(|a| normalize_alias(a))
            .collect::<BTreeSet<_>>();
        ProtonInstall {
            name: name.to_string(),
            path: PathBuf::from(format!("/test/{name}/proton")),
            is_official: false,
            aliases: aliases_vec,
            normalized_aliases: normalized,
        }
    }

    #[test]
    fn empty_community_version_returns_unknown() {
        let result = match_community_version("", &[]);
        assert_eq!(result.status, ProtonUpMatchStatus::Unknown);
        assert!(result.community_version.is_none());
        assert!(result.matched_install_name.is_none());
        assert!(result.recommended_version.is_none());
    }

    #[test]
    fn whitespace_only_community_version_returns_unknown() {
        let result = match_community_version("   ", &[]);
        assert_eq!(result.status, ProtonUpMatchStatus::Unknown);
        assert!(result.community_version.is_none());
        assert!(result.matched_install_name.is_none());
        assert!(result.recommended_version.is_none());
    }

    #[test]
    fn exact_alias_match_returns_matched() {
        let installs = vec![test_install(
            "GE-Proton9-4",
            &["GE-Proton9-4", "GE Proton 9-4"],
        )];
        let result = match_community_version("GE-Proton9-4", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Matched);
        assert_eq!(result.matched_install_name.as_deref(), Some("GE-Proton9-4"));
        assert_eq!(result.community_version.as_deref(), Some("GE-Proton9-4"));
        assert!(result.recommended_version.is_none());
    }

    #[test]
    fn normalized_match_returns_matched() {
        // "GE-Proton 9-4" normalizes to "geproton94", which matches "GE-Proton9-4"
        let installs = vec![test_install("GE-Proton9-4", &["GE-Proton9-4"])];
        let result = match_community_version("GE-Proton 9-4", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Matched);
        assert_eq!(result.matched_install_name.as_deref(), Some("GE-Proton9-4"));
        assert_eq!(result.community_version.as_deref(), Some("GE-Proton 9-4"));
        assert!(result.recommended_version.is_none());
    }

    #[test]
    fn no_match_returns_missing() {
        let installs = vec![test_install("GE-Proton9-4", &["GE-Proton9-4"])];
        let result = match_community_version("Proton Experimental", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Missing);
        assert!(result.matched_install_name.is_none());
        assert_eq!(
            result.community_version.as_deref(),
            Some("Proton Experimental")
        );
        assert_eq!(
            result.recommended_version.as_deref(),
            Some("Proton Experimental")
        );
    }

    #[test]
    fn case_insensitive_matching() {
        let installs = vec![test_install("GE-Proton9-4", &["GE-Proton9-4"])];
        let result = match_community_version("ge-proton9-4", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Matched);
        assert_eq!(result.matched_install_name.as_deref(), Some("GE-Proton9-4"));
    }

    #[test]
    fn multiple_installs_first_match_wins() {
        // Both installs have aliases that normalize to the same value.
        // The first one in the slice is returned.
        let installs = vec![
            test_install("GE-Proton9-21", &["GE-Proton9-21"]),
            test_install("GE-Proton9-21-Alt", &["GE-Proton9-21"]),
        ];
        // Exact alias match — finds the first install that has "GE-Proton9-21" as an alias.
        let result = match_community_version("GE-Proton9-21", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Matched);
        assert_eq!(
            result.matched_install_name.as_deref(),
            Some("GE-Proton9-21")
        );
    }

    #[test]
    fn community_version_with_special_characters_normalizes_correctly() {
        // The parenthetical and space characters are stripped during normalization.
        // "GE-Proton9-21 (beta)" normalizes to "geproton921beta".
        // We install a version whose alias also normalizes to "geproton921beta".
        let installs = vec![test_install(
            "GE-Proton9-21-beta",
            &["GE-Proton9-21-beta", "GEProton921beta"],
        )];
        let result = match_community_version("GE-Proton9-21 (beta)", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Matched);
        assert_eq!(
            result.matched_install_name.as_deref(),
            Some("GE-Proton9-21-beta")
        );
    }

    #[test]
    fn numeric_only_version_does_not_match_unrelated_installs() {
        // A bare number like "9" normalizes to "9".
        // It should NOT match "GE-Proton9-21" whose normalized form is "geproton921".
        let installs = vec![test_install("GE-Proton9-21", &["GE-Proton9-21"])];
        let result = match_community_version("9", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Missing);
        assert_eq!(result.community_version.as_deref(), Some("9"));
    }

    #[test]
    fn partial_version_does_not_match_longer_version() {
        // "GE-Proton9" normalizes to "geproton9".
        // "GE-Proton9-21" normalizes to "geproton921".
        // They are distinct normalized strings — no match expected.
        // (Normalization strips punctuation but not digits, so "9" != "921".)
        let installs = vec![test_install("GE-Proton9-21", &["GE-Proton9-21"])];
        let result = match_community_version("GE-Proton9", &installs);
        assert_eq!(result.status, ProtonUpMatchStatus::Missing);
        assert_eq!(result.community_version.as_deref(), Some("GE-Proton9"));
        assert_eq!(result.recommended_version.as_deref(), Some("GE-Proton9"));
    }
}
