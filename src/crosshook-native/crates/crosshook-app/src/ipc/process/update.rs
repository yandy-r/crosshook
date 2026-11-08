//! `update-log` / `update-complete` events of an update run.

use super::batch_lines;
use crate::ipc::WireEvent;
use crate::services::{RunEvent, UpdateCompletion};

/// Event carrying one updater log line (a bare JSON string).
pub const UPDATE_LOG_EVENT: &str = "update-log";
/// Event carrying the updater exit code (a bare JSON number or `null`).
pub const UPDATE_COMPLETE_EVENT: &str = "update-complete";

/// The frozen `update-complete` payload: the exit code, or `null` when the
/// updater was ended by a signal or never ran (the reason is in its log).
pub fn complete_payload(completion: Option<UpdateCompletion>) -> Option<i32> {
    completion.and_then(|completion| completion.code)
}

/// The wire events one update run event produces.
pub fn wire_events(event: RunEvent<UpdateCompletion>) -> Vec<WireEvent> {
    match event {
        RunEvent::Log(batch) => batch_lines(batch)
            .iter()
            .filter_map(|line| WireEvent::new(UPDATE_LOG_EVENT, line))
            .collect(),
        RunEvent::Finished(completion) => {
            WireEvent::new(UPDATE_COMPLETE_EVENT, &complete_payload(completion))
                .into_iter()
                .collect()
        }
    }
}

#[cfg(test)]
mod tests;
