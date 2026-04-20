pub mod exit_codes;
pub mod models;
pub mod patterns;

use std::collections::HashSet;
use std::process::ExitStatus;

use chrono::Utc;

use crate::launch::request::{ValidationSeverity, METHOD_STEAM_APPLAUNCH};

pub use models::{
    ActionableSuggestion, DiagnosticReport, ExitCodeInfo, FailureMode, PatternMatch,
    MAX_DIAGNOSTIC_ENTRIES, MAX_LINE_DISPLAY_CHARS, MAX_LOG_TAIL_BYTES,
};

pub fn analyze(exit_status: Option<ExitStatus>, log_tail: &str, method: &str) -> DiagnosticReport {
    let exit_info = exit_codes::analyze_exit_status(exit_status, method);
    let pattern_matches =
        filter_pattern_matches(patterns::scan_log_patterns(log_tail, method), &exit_info);
    let suggestions = build_suggestions(&exit_info, &pattern_matches, method);
    let severity = overall_severity(exit_info.severity, &pattern_matches);
    let summary = build_summary(&exit_info, &pattern_matches, method);

    DiagnosticReport {
        severity,
        summary,
        exit_info,
        pattern_matches,
        suggestions,
        launch_method: method.to_string(),
        log_tail_path: None,
        analyzed_at: Utc::now().to_rfc3339(),
        teardown_reason: None,
    }
}

pub fn should_surface_report(report: &DiagnosticReport) -> bool {
    if report.launch_method == METHOD_STEAM_APPLAUNCH && !report.pattern_matches.is_empty() {
        return true;
    }

    !matches!(
        report.exit_info.failure_mode,
        FailureMode::CleanExit | FailureMode::Indeterminate
    )
}

fn filter_pattern_matches(
    pattern_matches: Vec<PatternMatch>,
    exit_info: &ExitCodeInfo,
) -> Vec<PatternMatch> {
    pattern_matches
        .into_iter()
        .filter(|pattern_match| {
            pattern_match.pattern_id != "wine_fixme_noise"
                || !matches!(
                    exit_info.failure_mode,
                    FailureMode::CleanExit | FailureMode::Indeterminate
                )
        })
        .collect()
}

fn build_suggestions(
    exit_info: &ExitCodeInfo,
    pattern_matches: &[PatternMatch],
    method: &str,
) -> Vec<ActionableSuggestion> {
    let mut seen = HashSet::new();
    let mut suggestions = Vec::new();

    for pattern_match in pattern_matches {
        if !seen.contains(&pattern_match.suggestion) {
            let description = pattern_match.suggestion.clone();
            seen.insert(description.clone());
            suggestions.push(ActionableSuggestion {
                title: pattern_match.summary.clone(),
                description,
                severity: pattern_match.severity,
            });
        }
    }

    if let Some(fallback) = fallback_suggestion(exit_info, method) {
        if !seen.contains(&fallback.description) {
            seen.insert(fallback.description.clone());
            suggestions.push(fallback);
        }
    }

    suggestions.truncate(MAX_DIAGNOSTIC_ENTRIES);
    suggestions
}

fn fallback_suggestion(exit_info: &ExitCodeInfo, method: &str) -> Option<ActionableSuggestion> {
    let description = match exit_info.failure_mode {
        FailureMode::CleanExit => return None,
        FailureMode::Indeterminate if method == METHOD_STEAM_APPLAUNCH => {
            "Steam launch helper exit codes are not authoritative. Check the helper log and Steam client state for the real failure.".to_string()
        }
        FailureMode::Indeterminate => {
            "CrossHook could not classify the failure from the exit status alone. Review the helper log tail for context.".to_string()
        }
        FailureMode::CommandNotFound => {
            "Verify that the configured executable and helper paths still exist and point to runnable files.".to_string()
        }
        FailureMode::PermissionDenied => {
            "Verify execute permissions on the selected executable and any helper scripts before retrying.".to_string()
        }
        FailureMode::Segfault
        | FailureMode::Abort
        | FailureMode::BusError
        | FailureMode::IllegalInstruction
        | FailureMode::FloatingPointException => {
            "The process crashed before a clean shutdown. Re-run with the current log, Proton version, and prefix configuration under review.".to_string()
        }
        FailureMode::Kill | FailureMode::BrokenPipe | FailureMode::Terminated => {
            "The process was interrupted externally. Check for supervisors, launch wrappers, or desktop-session shutdowns that may have stopped it.".to_string()
        }
        FailureMode::NonZeroExit => {
            "The process returned a non-zero exit code without a recognized log signature. Review the helper log tail for the last error lines.".to_string()
        }
        FailureMode::UnknownSignal | FailureMode::Unknown => {
            "CrossHook could not map the failure to a known pattern. Review the helper log tail and recent system logs for more context.".to_string()
        }
    };

    Some(ActionableSuggestion {
        title: "Next step".to_string(),
        description,
        severity: exit_info.severity,
    })
}

fn overall_severity(
    exit_severity: ValidationSeverity,
    pattern_matches: &[PatternMatch],
) -> ValidationSeverity {
    pattern_matches
        .iter()
        .fold(exit_severity, |current, pattern_match| {
            if severity_rank(pattern_match.severity) < severity_rank(current) {
                pattern_match.severity
            } else {
                current
            }
        })
}

fn build_summary(
    exit_info: &ExitCodeInfo,
    pattern_matches: &[PatternMatch],
    method: &str,
) -> String {
    match pattern_matches.len() {
        0 if method == METHOD_STEAM_APPLAUNCH && exit_info.failure_mode == FailureMode::Indeterminate => {
            "Steam accepted the launch request, but no known failure signatures were detected in the helper log.".to_string()
        }
        0 => exit_info.description.clone(),
        1 => format!(
            "{} Matched 1 known issue: {}.",
            exit_info.description, pattern_matches[0].summary
        ),
        count => format!(
            "{} Matched {count} known issues; the highest priority match is {}.",
            exit_info.description, pattern_matches[0].summary
        ),
    }
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
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use crate::launch::{
        ValidationSeverity, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
    };

    use super::{analyze, should_surface_report, FailureMode};

    fn exit_status_from_code(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code << 8)
    }

    fn exit_status_from_signal(signal: i32, core_dumped: bool) -> ExitStatus {
        ExitStatus::from_raw(signal | if core_dumped { 0x80 } else { 0 })
    }

    #[test]
    fn analyze_combines_exit_status_patterns_and_suggestions() {
        let report = analyze(
            Some(exit_status_from_signal(11, true)),
            "err:module:import_dll Library ntdll.dll not found",
            METHOD_PROTON_RUN,
        );

        assert_eq!(report.severity, ValidationSeverity::Fatal);
        assert_eq!(report.exit_info.failure_mode, FailureMode::Segfault);
        assert_eq!(report.pattern_matches.len(), 1);
        assert_eq!(report.pattern_matches[0].pattern_id, "wine_ntdll_missing");
        assert!(!report.suggestions.is_empty());
        assert!(report.summary.contains("Matched 1 known issue"));
        assert_eq!(report.launch_method, METHOD_PROTON_RUN);
        assert!(chrono::DateTime::parse_from_rfc3339(&report.analyzed_at).is_ok());
    }

    #[test]
    fn analyze_keeps_steam_clean_exit_indeterminate_without_patterns() {
        let report = analyze(Some(exit_status_from_code(0)), "", METHOD_STEAM_APPLAUNCH);

        assert_eq!(report.severity, ValidationSeverity::Info);
        assert_eq!(report.exit_info.failure_mode, FailureMode::Indeterminate);
        assert!(report.pattern_matches.is_empty());
        assert_eq!(report.suggestions.len(), 1);
        assert!(report.summary.contains("Steam accepted the launch request"));
    }

    #[test]
    fn analyze_uses_exit_status_when_no_patterns_match() {
        let report = analyze(Some(exit_status_from_code(126)), "", METHOD_NATIVE);

        assert_eq!(report.severity, ValidationSeverity::Fatal);
        assert_eq!(report.exit_info.failure_mode, FailureMode::PermissionDenied);
        assert!(report.pattern_matches.is_empty());
        assert_eq!(
            report.summary,
            "The launched command could not be executed due to permissions."
        );
        assert_eq!(report.suggestions[0].severity, ValidationSeverity::Fatal);
    }

    #[test]
    fn clean_exit_fixme_noise_is_not_surfaceable() {
        let report = analyze(
            Some(exit_status_from_code(0)),
            "fixme:heap:HeapSetInformation unsupported flag 2",
            METHOD_PROTON_RUN,
        );

        assert!(report.pattern_matches.is_empty());
        assert!(!should_surface_report(&report));
    }

    #[test]
    fn steam_pattern_match_is_surfaceable_even_with_zero_exit() {
        let report = analyze(
            Some(exit_status_from_code(0)),
            "Steam is not running",
            METHOD_STEAM_APPLAUNCH,
        );

        assert_eq!(report.pattern_matches.len(), 1);
        assert!(should_surface_report(&report));
    }
}
