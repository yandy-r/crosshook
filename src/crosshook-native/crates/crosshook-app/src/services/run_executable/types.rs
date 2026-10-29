//! Wire, completion and error types of the run-executable service.

use std::fmt;

use crosshook_core::run_executable::{RunExecutableResult, RunExecutableValidationError};
use serde::{Deserialize, Serialize};

use crate::AppError;

/// The `run_executable` response: the launch result plus the run id that
/// tags the run's events (additive `run_id`, D10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunExecutableResponse {
    /// The launch result, flattened so the wire keeps its old fields.
    #[serde(flatten)]
    pub result: RunExecutableResult,
    /// The job id of the run.
    pub run_id: String,
}

/// How the executable run ended.
///
/// `code` is set for a natural exit and `signal` when the process was
/// terminated by a signal (cancel: `SIGTERM`, stop: `SIGKILL`). Both are
/// `None` when the process could not be spawned or supervised; the error
/// text is then in the run's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunCompletion {
    /// Exit code of a natural exit.
    pub code: Option<i32>,
    /// Signal number that terminated the process.
    pub signal: Option<i32>,
}

/// Why a run could not be validated or started.
///
/// Validation failures keep their typed variant so the presenter can place
/// the error on its field; everything else is a sanitized [`AppError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunExecutableStartError {
    /// The request failed validation; nothing ran.
    Validation(RunExecutableValidationError),
    /// Any other failure (prefix provisioning, log creation, busy prefix,
    /// no runtime); nothing ran.
    Failed(AppError),
}

impl fmt::Display for RunExecutableStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => f.write_str(&error.message()),
            Self::Failed(error) => f.write_str(error.message()),
        }
    }
}

impl std::error::Error for RunExecutableStartError {}

impl From<RunExecutableValidationError> for RunExecutableStartError {
    fn from(error: RunExecutableValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<AppError> for RunExecutableStartError {
    fn from(error: AppError) -> Self {
        Self::Failed(error)
    }
}
