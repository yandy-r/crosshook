//! Wire DTOs for the launch Tauri events, plus the per-session relay state.
//!
//! Moved verbatim (modulo module path) from the old `commands::launch::shared`:
//! the gamescope `xdg_backend` suppression is a UI concern and lives here, in
//! the relay, not in the launch service (feature-spec S2).

use crosshook_app::services::launch::{InjectionLevel, InjectionLogData, InjectionSource};
use crosshook_core::launch::{DiagnosticReport, SessionId, SessionKind};
use serde::Serialize;

pub(crate) const GAMESCOPE_XDG_BACKEND_SOURCE_MARKER: &str = "xdg_backend:";
pub(crate) const GAMESCOPE_XDG_BACKEND_MESSAGE_MARKER: &str =
    "Compositor released us but we were not acquired";
pub(crate) const GAMESCOPE_XDG_BACKEND_SUPPRESSION_NOTICE: &str =
    "[crosshook] Suppressing repeated gamescope xdg_backend console noise. The raw launch log still contains every line.";

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Default)]
pub(crate) struct LaunchLogRelayState {
    pub(crate) gamescope_xdg_backend_seen: bool,
    pub(crate) gamescope_xdg_backend_suppressed: usize,
    pub(crate) suppression_notice_emitted: bool,
}

pub(crate) fn is_gamescope_xdg_backend_line(line: &str) -> bool {
    line.contains(GAMESCOPE_XDG_BACKEND_SOURCE_MARKER)
        && line.contains(GAMESCOPE_XDG_BACKEND_MESSAGE_MARKER)
}

pub(crate) fn transform_launch_log_line_for_ui(
    state: &mut LaunchLogRelayState,
    line: &str,
) -> Vec<String> {
    if !is_gamescope_xdg_backend_line(line) {
        return vec![line.to_string()];
    }

    if !state.gamescope_xdg_backend_seen {
        state.gamescope_xdg_backend_seen = true;
        return vec![line.to_string()];
    }

    state.gamescope_xdg_backend_suppressed += 1;
    if !state.suppression_notice_emitted {
        state.suppression_notice_emitted = true;
        return vec![GAMESCOPE_XDG_BACKEND_SUPPRESSION_NOTICE.to_string()];
    }

    Vec::new()
}

pub(crate) fn suppression_summary_line(state: &LaunchLogRelayState) -> Option<String> {
    (state.gamescope_xdg_backend_suppressed > 0).then(|| {
        format!(
            "[crosshook] Suppressed {} repeated gamescope xdg_backend lines from the live console. See the raw launch log for full output.",
            state.gamescope_xdg_backend_suppressed
        )
    })
}

/// `launch-log` payload: one event per non-empty UI line.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct LaunchLogPayload<'a> {
    pub(crate) line: &'a str,
    pub(crate) session_id: &'a str,
}

/// `launch-diagnostic` payload: the report flattened next to the session id,
/// built at emit time so `DiagnosticReport` (and its persisted form) is
/// untouched.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct LaunchDiagnosticPayload<'a> {
    #[serde(flatten)]
    pub(crate) report: &'a DiagnosticReport,
    pub(crate) session_id: &'a str,
}

/// `launch-complete` payload. `code`/`signal` stay JSON `null` when absent
/// (no `skip_serializing_if`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct LaunchCompletePayload {
    pub(crate) code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) session_id: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InjectionLogLevel {
    Info,
    Warning,
    Error,
}

impl From<InjectionLevel> for InjectionLogLevel {
    fn from(value: InjectionLevel) -> Self {
        match value {
            InjectionLevel::Info => Self::Info,
            InjectionLevel::Warning => Self::Warning,
            InjectionLevel::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InjectionLogSource {
    Trainer,
    Injection,
    Runtime,
}

impl From<InjectionSource> for InjectionLogSource {
    fn from(value: InjectionSource) -> Self {
        match value {
            InjectionSource::Trainer => Self::Trainer,
            InjectionSource::Injection => Self::Injection,
            InjectionSource::Runtime => Self::Runtime,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InjectionLogSessionKind {
    Game,
    Trainer,
}

impl From<SessionKind> for InjectionLogSessionKind {
    fn from(value: SessionKind) -> Self {
        match value {
            SessionKind::Game => Self::Game,
            SessionKind::Trainer => Self::Trainer,
        }
    }
}

/// Display-safe structured payload for the runtime-only `injection-log` event.
///
/// Keep `message` scoped and sanitized. Do not populate it with raw helper
/// output, environment dumps, or unsanitized filesystem paths.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct InjectionLogEvent {
    pub(crate) timestamp: String,
    pub(crate) profile_name: String,
    pub(crate) session_id: String,
    pub(crate) session_kind: InjectionLogSessionKind,
    pub(crate) level: InjectionLogLevel,
    pub(crate) source: InjectionLogSource,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hook_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hook_name: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) unsupported_runtime: bool,
}

impl InjectionLogEvent {
    /// Builds the unchanged wire shape from service row data; `at` is
    /// formatted RFC 3339 here (the service carries `SystemTime`, S4).
    pub(crate) fn from_data(session_id: SessionId, data: &InjectionLogData) -> Self {
        let trimmed = data.profile_name.trim();
        Self {
            timestamp: chrono::DateTime::<chrono::Utc>::from(data.at).to_rfc3339(),
            profile_name: if trimmed.is_empty() {
                "Unknown profile".to_string()
            } else {
                trimmed.to_string()
            },
            session_id: session_id.to_string(),
            session_kind: data.session_kind.into(),
            level: data.level.into(),
            source: data.source.into(),
            message: data.message.clone(),
            hook_id: None,
            hook_name: None,
            unsupported_runtime: data.unsupported_runtime,
        }
    }
}
