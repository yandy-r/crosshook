//! Wire and completion types of the update service.

use crosshook_core::update::UpdateGameResult;
use serde::{Deserialize, Serialize};

use crate::services::process::ProcessCompletion;

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

/// How the updater run ended (see [`ProcessCompletion`]).
pub type UpdateCompletion = ProcessCompletion;
