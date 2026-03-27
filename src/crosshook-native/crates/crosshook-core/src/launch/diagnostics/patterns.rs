use std::collections::HashSet;

use crate::launch::request::ValidationSeverity;

use super::models::{
    FailureMode, FailurePatternDef, PatternMatch, MAX_DIAGNOSTIC_ENTRIES, MAX_LINE_DISPLAY_CHARS,
};

const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDef] = &[
    FailurePatternDef {
        id: "wine_ntdll_missing",
        markers: &["ntdll.dll not found"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Fatal,
        summary: "WINE could not find ntdll.dll.",
        suggestion: "The prefix looks incomplete. Recreate the prefix or reinstall the Proton version.",
        applies_to_methods: &["proton_run"],
    },
    FailurePatternDef {
        id: "wine_vulkan_init_fail",
        markers: &["Failed to init Vulkan", "winevulkan"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Fatal,
        summary: "WINE failed to initialize Vulkan.",
        suggestion: "Check that Vulkan drivers are installed and the selected Proton build can access the GPU.",
        applies_to_methods: &["proton_run"],
    },
    FailurePatternDef {
        id: "wine_prefix_missing",
        markers: &["WINEPREFIX", "does not exist"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Fatal,
        summary: "The configured WINE prefix path is missing.",
        suggestion: "Verify the prefix path in the profile and recreate it if the directory was moved or deleted.",
        applies_to_methods: &["proton_run"],
    },
    FailurePatternDef {
        id: "proton_version_mismatch",
        markers: &["Proton: No compatibility tool"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Fatal,
        summary: "Steam could not find the requested compatibility tool.",
        suggestion: "Select a Proton version that is installed locally or refresh Steam's compatibility tool list.",
        applies_to_methods: &["steam_applaunch"],
    },
    FailurePatternDef {
        id: "steam_not_running",
        markers: &["Steam is not running"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Fatal,
        summary: "Steam was not available for the launch helper.",
        suggestion: "Start Steam and sign in before launching again.",
        applies_to_methods: &["steam_applaunch"],
    },
    FailurePatternDef {
        id: "permission_denied",
        markers: &["Permission denied"],
        failure_mode: FailureMode::PermissionDenied,
        severity: ValidationSeverity::Fatal,
        summary: "The executable could not be started due to a permission error.",
        suggestion: "Check file permissions on the game or trainer executable and ensure it is runnable.",
        applies_to_methods: &[],
    },
    FailurePatternDef {
        id: "exe_not_found",
        markers: &["cannot find", "No such file"],
        failure_mode: FailureMode::CommandNotFound,
        severity: ValidationSeverity::Fatal,
        summary: "The requested executable could not be found.",
        suggestion: "Verify the executable path in the profile and confirm the file still exists.",
        applies_to_methods: &[],
    },
    FailurePatternDef {
        id: "wine_crash_dump",
        markers: &["Unhandled exception", "backtrace:"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Warning,
        summary: "WINE reported an unhandled exception.",
        suggestion: "Review the log tail and the current Proton version; the prefix or executable may be crashing.",
        applies_to_methods: &["proton_run"],
    },
    FailurePatternDef {
        id: "dxvk_state_cache",
        markers: &["DXVK: State cache"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Info,
        summary: "DXVK state cache activity was logged.",
        suggestion: "This is informational and usually not actionable by itself.",
        applies_to_methods: &["proton_run"],
    },
    FailurePatternDef {
        id: "wine_fixme_noise",
        markers: &["fixme:"],
        failure_mode: FailureMode::Indeterminate,
        severity: ValidationSeverity::Info,
        summary: "WINE emitted fixme output.",
        suggestion: "This is usually diagnostic noise unless it appears alongside a non-zero exit or another failure pattern.",
        applies_to_methods: &["proton_run"],
    },
];

pub fn scan_log_patterns(log_tail: &str, method: &str) -> Vec<PatternMatch> {
    scan_log_patterns_with_definitions(log_tail, method, FAILURE_PATTERN_DEFINITIONS)
}

fn scan_log_patterns_with_definitions(
    log_tail: &str,
    method: &str,
    definitions: &[FailurePatternDef],
) -> Vec<PatternMatch> {
    let mut matches = Vec::new();

    for definition in definitions {
        if !applies_to_method(definition, method) {
            continue;
        }

        let Some(matched_line) = find_matching_line(log_tail, definition.markers) else {
            continue;
        };

        matches.push(PatternMatch {
            pattern_id: definition.id.to_string(),
            summary: definition.summary.to_string(),
            severity: definition.severity,
            matched_line: Some(truncate_line(&matched_line)),
            suggestion: definition.suggestion.to_string(),
        });
    }

    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(matches.len());
    for pattern_match in matches {
        if seen.insert(pattern_match.pattern_id.clone()) {
            deduped.push(pattern_match);
        }
    }

    deduped.sort_by_key(|pattern_match| severity_rank(pattern_match.severity));
    deduped.truncate(MAX_DIAGNOSTIC_ENTRIES);
    deduped
}

fn applies_to_method(definition: &FailurePatternDef, method: &str) -> bool {
    definition.applies_to_methods.is_empty() || definition.applies_to_methods.contains(&method)
}

fn find_matching_line(log_tail: &str, markers: &[&str]) -> Option<String> {
    log_tail
        .lines()
        .find(|line| markers.iter().any(|marker| line.contains(marker)))
        .map(|line| line.to_string())
}

fn truncate_line(line: &str) -> String {
    line.chars().take(MAX_LINE_DISPLAY_CHARS).collect()
}

fn severity_rank(severity: ValidationSeverity) -> u8 {
    match severity {
        ValidationSeverity::Fatal => 0,
        ValidationSeverity::Warning => 1,
        ValidationSeverity::Info => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_have_required_fields() {
        assert_eq!(FAILURE_PATTERN_DEFINITIONS.len(), 10);

        for definition in FAILURE_PATTERN_DEFINITIONS {
            assert!(!definition.id.is_empty());
            assert!(!definition.markers.is_empty());
            assert!(!definition.suggestion.is_empty());
        }
    }

    #[test]
    fn matches_wine_fixture_lines() {
        let log_tail = "\
err:module:import_dll Library ntdll.dll not found\n\
DXVK: State cache loaded successfully\n";

        let matches = scan_log_patterns(log_tail, "proton_run");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].pattern_id, "wine_ntdll_missing");
        assert_eq!(
            matches[0].matched_line.as_deref(),
            Some("err:module:import_dll Library ntdll.dll not found")
        );
        assert_eq!(matches[1].pattern_id, "dxvk_state_cache");
    }

    #[test]
    fn filters_patterns_by_method() {
        let log_tail = "Steam is not running\nFailed to init Vulkan\n";

        let steam_matches = scan_log_patterns(log_tail, "steam_applaunch");
        assert_eq!(steam_matches.len(), 1);
        assert_eq!(steam_matches[0].pattern_id, "steam_not_running");

        let native_matches = scan_log_patterns(log_tail, "native");
        assert!(native_matches.is_empty());
    }

    #[test]
    fn deduplicates_by_pattern_id() {
        let marker_one: &'static str = Box::leak(String::from("marker-one").into_boxed_str());
        let marker_two: &'static str = Box::leak(String::from("marker-two").into_boxed_str());
        let markers_one: &'static [&'static str] = Box::leak(vec![marker_one].into_boxed_slice());
        let markers_two: &'static [&'static str] = Box::leak(vec![marker_two].into_boxed_slice());
        let methods: &'static [&'static str] = Box::leak(Vec::new().into_boxed_slice());
        let definitions = vec![
            FailurePatternDef {
                id: "duplicate_id",
                markers: markers_one,
                failure_mode: FailureMode::Unknown,
                severity: ValidationSeverity::Warning,
                summary: "first",
                suggestion: "first suggestion",
                applies_to_methods: methods,
            },
            FailurePatternDef {
                id: "duplicate_id",
                markers: markers_two,
                failure_mode: FailureMode::Unknown,
                severity: ValidationSeverity::Fatal,
                summary: "second",
                suggestion: "second suggestion",
                applies_to_methods: methods,
            },
        ];

        let log_tail = "marker-one\nmarker-two\n";
        let matches = scan_log_patterns_with_definitions(log_tail, "proton_run", &definitions);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].summary, "first");
    }

    #[test]
    fn caps_the_number_of_matches() {
        let log_tail = (0..60)
            .map(|index| format!("needle-{index}"))
            .collect::<Vec<_>>()
            .join(" ");

        let definitions = (0..60)
            .map(|index| {
                let id: &'static str = Box::leak(format!("pattern-{index}").into_boxed_str());
                let marker: &'static str = Box::leak(format!("needle-{index}").into_boxed_str());
                let summary: &'static str = Box::leak(format!("summary-{index}").into_boxed_str());
                let suggestion: &'static str =
                    Box::leak(format!("suggestion-{index}").into_boxed_str());
                let markers: &'static [&'static str] = Box::leak(vec![marker].into_boxed_slice());
                let methods: &'static [&'static str] = Box::leak(Vec::new().into_boxed_slice());

                FailurePatternDef {
                    id,
                    markers,
                    failure_mode: FailureMode::Unknown,
                    severity: ValidationSeverity::Info,
                    summary,
                    suggestion,
                    applies_to_methods: methods,
                }
            })
            .collect::<Vec<_>>();

        let matches = scan_log_patterns_with_definitions(&log_tail, "proton_run", &definitions);

        assert_eq!(matches.len(), MAX_DIAGNOSTIC_ENTRIES);
        assert_eq!(matches[0].pattern_id, "pattern-0");
        assert_eq!(
            matches.last().map(|entry| entry.pattern_id.as_str()),
            Some("pattern-49")
        );
    }
}
