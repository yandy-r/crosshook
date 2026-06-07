use std::sync::Arc;

use crosshook_core::launch::{
    LaunchHookExecutionContext, LaunchSessionRegistry, LaunchValidationIssue, SessionId,
    SessionKind, WatchdogOutcome,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::LaunchHook;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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

#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<LaunchValidationIssue>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InjectionLogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InjectionLogSource {
    Trainer,
    Injection,
    Runtime,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_injection_log_event(
    timestamp: impl Into<String>,
    profile_name: Option<&str>,
    session_id: SessionId,
    session_kind: SessionKind,
    level: InjectionLogLevel,
    source: InjectionLogSource,
    message: impl Into<String>,
    unsupported_runtime: bool,
) -> InjectionLogEvent {
    InjectionLogEvent {
        timestamp: timestamp.into(),
        profile_name: profile_name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Unknown profile")
            .to_string(),
        session_id: session_id.to_string(),
        session_kind: session_kind.into(),
        level,
        source,
        message: message.into(),
        hook_id: None,
        hook_name: None,
        unsupported_runtime,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_injection_log_event(
    app: &AppHandle,
    profile_name: Option<&str>,
    session_id: SessionId,
    session_kind: SessionKind,
    level: InjectionLogLevel,
    source: InjectionLogSource,
    message: impl Into<String>,
    unsupported_runtime: bool,
) {
    let event = build_injection_log_event(
        chrono::Utc::now().to_rfc3339(),
        profile_name,
        session_id,
        session_kind,
        level,
        source,
        message,
        unsupported_runtime,
    );

    if let Err(error) = app.emit("injection-log", event) {
        tracing::warn!(%error, "failed to emit injection-log event");
    }
}

/// Context plumbed from a launch command into the log-stream task so the
/// stream finalizer can persist diagnostics and reconcile launch-session
/// lifecycle. `session_id`, `session_kind`, and `session_registry` are
/// required — every launch registers with the session registry up front,
/// so these carry no "session might not exist" optionality.
#[derive(Clone)]
pub(crate) struct LaunchStreamContext {
    pub(crate) metadata_store: MetadataStore,
    pub(crate) operation_id: Option<String>,
    pub(crate) steam_app_id: String,
    pub(crate) trainer_host_path: Option<String>,
    pub(crate) profile_name: Option<String>,
    pub(crate) steam_client_path: String,
    pub(crate) watchdog_outcome: WatchdogOutcome,
    pub(crate) session_id: SessionId,
    pub(crate) session_kind: SessionKind,
    pub(crate) session_registry: Arc<LaunchSessionRegistry>,
    pub(crate) hook_context: LaunchHookStreamContext,
}

#[derive(Clone)]
pub(crate) struct LaunchHookStreamContext {
    pub(crate) post_exit_hooks: Vec<LaunchHook>,
    pub(crate) execution_context: LaunchHookExecutionContext,
}
