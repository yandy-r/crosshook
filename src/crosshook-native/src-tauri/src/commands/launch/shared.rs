use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crosshook_core::launch::LaunchValidationIssue;
use crosshook_core::metadata::MetadataStore;
use serde::Serialize;

pub(crate) const GAMESCOPE_XDG_BACKEND_SOURCE_MARKER: &str = "xdg_backend:";
pub(crate) const GAMESCOPE_XDG_BACKEND_MESSAGE_MARKER: &str =
    "Compositor released us but we were not acquired";
pub(crate) const GAMESCOPE_XDG_BACKEND_SUPPRESSION_NOTICE: &str =
    "[crosshook] Suppressing repeated gamescope xdg_backend console noise. The raw launch log still contains every line.";

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

#[derive(Clone)]
pub(crate) struct LaunchStreamContext {
    pub(crate) metadata_store: MetadataStore,
    pub(crate) operation_id: Option<String>,
    pub(crate) steam_app_id: String,
    pub(crate) trainer_host_path: Option<String>,
    pub(crate) profile_name: Option<String>,
    pub(crate) steam_client_path: String,
    pub(crate) watchdog_killed: Arc<AtomicBool>,
}
