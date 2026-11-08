//! `launch_game` / `launch_trainer` result.

use crosshook_core::launch::LaunchValidationIssue;
use serde::Serialize;

use crate::services::launch::{LaunchEvents, LaunchSession};

/// IPC result of a started launch.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    /// Always `true`: a failed start is an `Err`, not a result.
    pub succeeded: bool,
    /// User-facing summary.
    pub message: String,
    /// Registry id of the started session; matches `session_id` of the
    /// `launch-session-changed` events and `list_running_sessions`.
    pub session_id: String,
    /// Path of the session log file.
    pub helper_log_path: String,
    /// Non-fatal warnings collected before the spawn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<LaunchValidationIssue>,
}

impl LaunchResult {
    /// Splits a started session into its IPC result and the event stream the
    /// shell relays.
    pub fn from_session(session: LaunchSession, message: &str) -> (Self, LaunchEvents) {
        let LaunchSession {
            id,
            log_path,
            warnings,
            events,
            ..
        } = session;
        let result = Self {
            succeeded: true,
            message: message.to_string(),
            session_id: id.to_string(),
            helper_log_path: log_path.to_string_lossy().into_owned(),
            warnings,
        };
        (result, events)
    }
}
