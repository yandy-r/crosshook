use crosshook_core::launch::LaunchValidationIssue;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    /// Registry id of the started session; matches `session_id` of the
    /// `launch-session-changed` events and `list_running_sessions`.
    pub session_id: String,
    pub helper_log_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<LaunchValidationIssue>,
}
