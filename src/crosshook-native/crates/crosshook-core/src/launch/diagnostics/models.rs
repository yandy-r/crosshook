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
