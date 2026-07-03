use serde::{Deserialize, Serialize};

use crate::launch::request::ValidationSeverity;
use crate::launch::session::TeardownReason;

pub const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_DIAGNOSTIC_ENTRIES: usize = 50;
pub const MAX_LINE_DISPLAY_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub severity: ValidationSeverity,
    pub summary: String,
    pub exit_info: ExitCodeInfo,
    pub pattern_matches: Vec<PatternMatch>,
    pub suggestions: Vec<ActionableSuggestion>,
    pub launch_method: String,
    pub log_tail_path: Option<String>,
    pub analyzed_at: String,
    /// Populated by the stream finalizer to record why this launch was torn
    /// down — set by the gamescope watchdog when it fires, or by the
    /// cancel-drain path for launches that have no gamescope tree to tear
    /// down (e.g. a non-gamescope trainer cascaded by its parent game).
    /// Optional for backward-compat with pre-#230
    /// `launch_operations.diagnostic_json` rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teardown_reason: Option<TeardownReason>,
    /// Mod-coexistence advisory codes that were active for this launch,
    /// capped at `MAX_COEXISTENCE_ADVISORY_RECORDS` entries. Empty for
    /// pre-#28 rows and launches without registered mods.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coexistence_advisories: Vec<CoexistenceAdvisoryRecord>,
}

/// Compact advisory record persisted in `launch_operations.diagnostic_json`
/// (codes + severity only — no prose, to respect `MAX_DIAGNOSTIC_JSON_BYTES`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoexistenceAdvisoryRecord {
    pub code: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitCodeInfo {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub signal_name: Option<String>,
    pub core_dumped: bool,
    pub failure_mode: FailureMode,
    pub description: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    CleanExit,
    NonZeroExit,
    Segfault,
    Abort,
    Kill,
    BusError,
    IllegalInstruction,
    FloatingPointException,
    BrokenPipe,
    Terminated,
    CommandNotFound,
    PermissionDenied,
    UnknownSignal,
    Indeterminate,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_id: String,
    pub summary: String,
    pub severity: ValidationSeverity,
    pub matched_line: Option<String>,
    pub suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionableSuggestion {
    pub title: String,
    pub description: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailurePatternDef {
    pub id: &'static str,
    pub markers: &'static [&'static str],
    pub failure_mode: FailureMode,
    pub severity: ValidationSeverity,
    pub summary: &'static str,
    pub suggestion: &'static str,
    pub applies_to_methods: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> DiagnosticReport {
        DiagnosticReport {
            severity: ValidationSeverity::Info,
            summary: "Clean exit".to_string(),
            exit_info: ExitCodeInfo {
                code: Some(0),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::CleanExit,
                description: "Process exited cleanly".to_string(),
                severity: ValidationSeverity::Info,
            },
            pattern_matches: vec![],
            suggestions: vec![],
            launch_method: "native".to_string(),
            log_tail_path: None,
            analyzed_at: "2026-01-01T00:00:00Z".to_string(),
            teardown_reason: None,
            coexistence_advisories: vec![],
        }
    }

    #[test]
    fn diagnostic_report_deserializes_without_coexistence_field() {
        let json = r#"{
            "severity": "info",
            "summary": "Clean exit",
            "exit_info": {
                "code": 0,
                "signal": null,
                "signal_name": null,
                "core_dumped": false,
                "failure_mode": "clean_exit",
                "description": "Process exited cleanly",
                "severity": "info"
            },
            "pattern_matches": [],
            "suggestions": [],
            "launch_method": "native",
            "log_tail_path": null,
            "analyzed_at": "2026-01-01T00:00:00Z"
        }"#;

        let report: DiagnosticReport = serde_json::from_str(json).unwrap();
        assert!(
            report.coexistence_advisories.is_empty(),
            "pre-#28 diagnostic_json rows must deserialize with empty advisories"
        );
    }

    #[test]
    fn diagnostic_report_round_trips_with_advisories() {
        let mut report = sample_report();
        report.coexistence_advisories = vec![
            CoexistenceAdvisoryRecord {
                code: "mod_coexistence_injection_vector".to_string(),
                severity: ValidationSeverity::Warning,
            },
            CoexistenceAdvisoryRecord {
                code: "mod_coexistence_file_replacement_notice".to_string(),
                severity: ValidationSeverity::Info,
            },
        ];

        let json = serde_json::to_string(&report).unwrap();
        let restored: DiagnosticReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, report);
    }

    #[test]
    fn empty_advisories_key_is_skipped_in_json() {
        let json = serde_json::to_string(&sample_report()).unwrap();
        assert!(
            !json.contains("coexistence_advisories"),
            "empty advisories must not bloat diagnostic_json: {json}"
        );
    }
}
