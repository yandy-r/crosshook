//! Wire and completion types of the update service.

use crosshook_core::update::UpdateGameResult;
use serde::{Deserialize, Serialize};

/// The `update_game` response: the launch result plus the run id that tags
/// the run's events (additive `run_id`, D2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateGameResponse {
    /// The launch result, flattened so the wire keeps its old fields.
    #[serde(flatten)]
    pub result: UpdateGameResult,
    /// The job id of the run.
    pub run_id: String,
}

/// How the updater run ended.
///
/// `code` is set for a natural exit and `signal` when the process was
/// terminated by a signal (cancel). Both are `None` when the updater could
/// not be spawned or supervised; the error text is then in the run's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateCompletion {
    /// Exit code of a natural exit.
    pub code: Option<i32>,
    /// Signal number that terminated the process.
    pub signal: Option<i32>,
}
