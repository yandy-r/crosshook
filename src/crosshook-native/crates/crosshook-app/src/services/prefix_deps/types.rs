//! Wire types of the prefix-dependency service (moved from the Tauri command
//! module; only additive fields).

use crosshook_core::prefix_deps::{PrefixDependencyInstallOutcome, PrefixVersionRestoreState};
use serde::{Deserialize, Serialize};

/// Restart-safe Windows compatibility repair status for a Wine prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixVersionRepairStatus {
    /// A journal row exists, so installs and launches stay blocked.
    pub required: bool,
    /// `Pending` or `Failed` while a row exists.
    pub state: Option<PrefixVersionRestoreState>,
    /// Sanitized error of the last failed restore.
    pub last_error: Option<String>,
    /// A job currently holds this prefix (install, repair or recovery), so a
    /// pending row is expected and not yet a repair request.
    pub in_progress: bool,
    /// Result of the last automatic (startup or pre-install) recovery of this
    /// prefix in this session.
    pub last_recovery: Option<LastRecovery>,
}

impl PrefixVersionRepairStatus {
    /// Status of a prefix without a journal row.
    pub(crate) fn clean() -> Self {
        Self {
            required: false,
            state: None,
            last_error: None,
            in_progress: false,
            last_recovery: None,
        }
    }
}

/// Last automatic recovery of a prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastRecovery {
    /// Whether the prefix was restored.
    pub outcome: RecoveryOutcome,
    /// Sanitized failure detail (`None` when restored).
    pub message: Option<String>,
}

/// Outcome of an automatic recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOutcome {
    /// The Windows version was restored and verified; the row is gone.
    Restored,
    /// Recovery failed; the prefix still needs repair.
    Failed,
}

/// Payload of a `prefix-dep-log` event: raw request names, sanitized line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepLogPayload {
    /// Profile name as requested.
    pub profile_name: String,
    /// Prefix path exactly as requested (listeners match on it).
    pub prefix_path: String,
    /// Sanitized output line.
    pub line: String,
}

/// Payload of a `prefix-dep-complete` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepCompletePayload {
    /// Profile name as requested.
    pub profile_name: String,
    /// Prefix path exactly as requested (listeners match on it).
    pub prefix_path: String,
    /// Install succeeded and the prefix's Windows version is safe.
    pub succeeded: bool,
    /// Legacy alias of `install_exit_code`.
    pub exit_code: Option<i32>,
    /// The install tool exited successfully.
    pub install_succeeded: bool,
    /// Exit code of the install tool.
    pub install_exit_code: Option<i32>,
    /// State of the Windows-version restore.
    pub restore_state: PrefixVersionRestoreState,
    /// Sanitized restore error.
    pub restore_error: Option<String>,
}

/// Typed completion of a prefix-dependency install job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepInstallCompletion {
    /// Install and restore results after journal resolution.
    pub outcome: PrefixDependencyInstallOutcome,
}

/// Builds the completion payload: `succeeded` requires a successful install
/// and a prefix whose Windows version is safe (`NotRequired` or `Succeeded`).
pub fn completion_payload(
    profile_name: impl Into<String>,
    raw_prefix_path: impl Into<String>,
    outcome: PrefixDependencyInstallOutcome,
) -> DepCompletePayload {
    let restore_safe = matches!(
        outcome.restore_state,
        PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Succeeded
    );
    DepCompletePayload {
        profile_name: profile_name.into(),
        prefix_path: raw_prefix_path.into(),
        succeeded: outcome.install_succeeded && restore_safe,
        exit_code: outcome.install_exit_code,
        install_succeeded: outcome.install_succeeded,
        install_exit_code: outcome.install_exit_code,
        restore_state: outcome.restore_state,
        restore_error: outcome.restore_error,
    }
}
