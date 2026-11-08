//! `prefix-dep-log` / `prefix-dep-complete` events of a dependency install.
//!
//! Payloads carry the raw request `profile_name`/`prefix_path` (listeners
//! match on the exact strings they sent).

use crosshook_core::prefix_deps::{PrefixDependencyInstallOutcome, PrefixVersionRestoreState};

use super::batch_lines;
use crate::ipc::WireEvent;
use crate::services::prefix_deps::{
    completion_payload, DepCompletePayload, DepInstallCompletion, DepLogPayload,
};
use crate::services::RunEvent;

/// Event carrying one install log line.
pub const PREFIX_DEP_LOG_EVENT: &str = "prefix-dep-log";
/// Event carrying the install outcome.
pub const PREFIX_DEP_COMPLETE_EVENT: &str = "prefix-dep-complete";

/// Shown when an install job ended without reporting a result.
pub const MSG_NO_RESULT: &str = "dependency install ended without a result";

/// The `prefix-dep-complete` payload of a finished install. A job that ended
/// without a completion is reported as a failed install whose restore is
/// still pending, so listeners stop waiting and the prefix is never reported
/// safe.
pub fn complete_payload(
    profile_name: &str,
    prefix_path: &str,
    completion: Option<DepInstallCompletion>,
) -> DepCompletePayload {
    let outcome = completion.map_or_else(
        || PrefixDependencyInstallOutcome {
            install_succeeded: false,
            install_exit_code: None,
            install_error: Some(MSG_NO_RESULT.to_string()),
            restore_state: PrefixVersionRestoreState::Pending,
            restore_error: Some(MSG_NO_RESULT.to_string()),
        },
        |done| done.outcome,
    );
    completion_payload(profile_name, prefix_path, outcome)
}

/// The wire events one install run event produces. The completion is only
/// produced after the service finished the job: final log drain, restore and
/// journal resolution included.
pub fn wire_events(
    profile_name: &str,
    prefix_path: &str,
    event: RunEvent<DepInstallCompletion>,
) -> Vec<WireEvent> {
    match event {
        RunEvent::Log(batch) => batch_lines(batch)
            .into_iter()
            .filter_map(|line| {
                let payload = DepLogPayload {
                    profile_name: profile_name.to_string(),
                    prefix_path: prefix_path.to_string(),
                    line,
                };
                WireEvent::new(PREFIX_DEP_LOG_EVENT, &payload)
            })
            .collect(),
        RunEvent::Finished(completion) => {
            let payload = complete_payload(profile_name, prefix_path, completion);
            WireEvent::new(PREFIX_DEP_COMPLETE_EVENT, &payload)
                .into_iter()
                .collect()
        }
    }
}

#[cfg(test)]
mod tests;
