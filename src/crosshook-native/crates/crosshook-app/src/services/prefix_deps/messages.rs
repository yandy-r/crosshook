//! User-facing texts of the prefix-dependency service and the mappers that
//! turn core errors into [`AppError`]s.

use crosshook_core::prefix_deps::PrefixDepsError;

use super::tools::ToolSelectionError;
use crate::{AppError, AppErrorKind};

/// Install refusal while offline (winetricks downloads its verbs).
pub const MSG_OFFLINE: &str = "offline mode is enabled";
/// Check refusal when no dependency tool is available.
pub const MSG_NO_TOOL_CHECK: &str =
    "No winetricks or protontricks binary found. Install winetricks or configure the path in Settings.";
/// Install refusal when no dependency tool is available.
pub const MSG_NO_TOOL_INSTALL: &str = "No winetricks or protontricks binary found.";
/// Refusal when only protontricks is available without a Steam App ID.
pub const MSG_APP_ID_REQUIRED: &str =
    "protontricks requires a valid nonzero Steam App ID; configure winetricks or set steam.app_id";
/// Install refusal while the prefix has an unresolved journal row.
pub const MSG_REPAIR_REQUIRED: &str =
    "This prefix requires Windows version repair before installing dependencies.";

/// Default journal error for a failed restore without detail.
pub(crate) const MSG_RESTORE_FAILED: &str = "Windows version restoration failed";
/// Journal error when a restore reports a state it may not report.
pub(crate) const MSG_RESTORE_INVALID_STATE: &str =
    "Windows version restoration returned an invalid state";

/// Conflict text of a second install on a busy prefix.
pub fn msg_already_installing(prefix: &str) -> String {
    format!("installation already in progress for prefix: {prefix}")
}

/// Journal error recorded when a verified restore could not delete its row.
pub(crate) fn msg_cleanup_failed(error: &str) -> String {
    format!("Windows version was restored, but repair journal cleanup failed: {error}")
}

/// Maps a tool-selection failure; `check` picks the longer check-time text.
pub(crate) fn tool_error(error: ToolSelectionError, check: bool) -> AppError {
    match error {
        ToolSelectionError::NotFound => AppError::new(
            AppErrorKind::Unavailable,
            if check {
                MSG_NO_TOOL_CHECK
            } else {
                MSG_NO_TOOL_INSTALL
            },
        ),
        ToolSelectionError::ProtontricksNeedsAppId => {
            AppError::validation("steam_app_id", MSG_APP_ID_REQUIRED)
        }
    }
}

/// Maps a core prefix-deps error, keeping its (path-sanitized) text.
pub(crate) fn deps_error(field: &'static str, error: PrefixDepsError) -> AppError {
    let message = error.to_string();
    match error {
        PrefixDepsError::ValidationError(_) | PrefixDepsError::PrefixNotInitialized { .. } => {
            AppError::validation(field, message)
        }
        PrefixDepsError::BinaryNotFound { .. } | PrefixDepsError::Timeout { .. } => {
            AppError::new(AppErrorKind::Unavailable, message)
        }
        PrefixDepsError::AlreadyInstalling { .. } => AppError::new(AppErrorKind::Conflict, message),
        PrefixDepsError::ProcessFailed { .. } | PrefixDepsError::Database { .. } => {
            AppError::new(AppErrorKind::Internal, message)
        }
    }
}
