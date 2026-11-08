//! Run-executable wire contract: the structured command error and the
//! `run-executable-log` / `run-executable-complete` events.

use crosshook_core::run_executable::RunExecutableValidationError;
use serde::Serialize;

use super::batch_lines;
use crate::ipc::WireEvent;
use crate::services::{RunCompletion, RunEvent, RunExecutableStartError};

/// Event carrying one log line of a run.
pub const RUN_EXECUTABLE_LOG_EVENT: &str = "run-executable-log";
/// Event carrying how a run ended.
pub const RUN_EXECUTABLE_COMPLETE_EVENT: &str = "run-executable-complete";

/// Structured error envelope returned to the frontend by every `run_executable`
/// command. The frontend matches on `kind` and uses `field` for input-level
/// error placement, removing the need for any message-text parsing.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunCommandError {
    /// The request failed validation; `field` names the offending input.
    Validation {
        /// The core validation variant.
        variant: RunExecutableValidationError,
        /// The request field the error is about.
        field: &'static str,
        /// User-facing message.
        message: String,
    },
    /// The run could not be started or supervised.
    Runtime {
        /// Sanitized failure text.
        message: String,
    },
}

impl From<RunExecutableValidationError> for RunCommandError {
    fn from(value: RunExecutableValidationError) -> Self {
        Self::Validation {
            field: value.field(),
            message: value.message(),
            variant: value,
        }
    }
}

impl From<RunExecutableStartError> for RunCommandError {
    fn from(value: RunExecutableStartError) -> Self {
        match value {
            RunExecutableStartError::Validation(variant) => Self::from(variant),
            RunExecutableStartError::Failed(error) => Self::Runtime {
                message: error.message().to_string(),
            },
        }
    }
}

/// `run-executable-log` payload: one log line of the run `run_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunExecutableLogPayload {
    /// The run the line belongs to.
    pub run_id: String,
    /// The console line.
    pub line: String,
}

/// `run-executable-complete` payload: how the run `run_id` ended. Both
/// fields are `null` when the process could not be spawned or supervised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunExecutableCompletePayload {
    /// The run that ended.
    pub run_id: String,
    /// Exit code, when the process exited.
    pub code: Option<i32>,
    /// Terminating signal, when it was signalled.
    pub signal: Option<i32>,
}

impl RunExecutableCompletePayload {
    /// Builds the payload for `run_id` from the run's completion, if any.
    pub fn new(run_id: String, completion: Option<RunCompletion>) -> Self {
        let (code, signal) = completion.map_or((None, None), |done| (done.code, done.signal));
        Self {
            run_id,
            code,
            signal,
        }
    }
}

/// The wire events one run event produces. The completion is only produced
/// after the service has finished the run (final log drain and
/// throwaway-prefix cleanup included).
pub fn wire_events(run_id: &str, event: RunEvent<RunCompletion>) -> Vec<WireEvent> {
    match event {
        RunEvent::Log(batch) => batch_lines(batch)
            .into_iter()
            .filter_map(|line| {
                let payload = RunExecutableLogPayload {
                    run_id: run_id.to_string(),
                    line,
                };
                WireEvent::new(RUN_EXECUTABLE_LOG_EVENT, &payload)
            })
            .collect(),
        RunEvent::Finished(completion) => {
            let payload = RunExecutableCompletePayload::new(run_id.to_string(), completion);
            WireEvent::new(RUN_EXECUTABLE_COMPLETE_EVENT, &payload)
                .into_iter()
                .collect()
        }
    }
}

#[cfg(test)]
mod tests;
