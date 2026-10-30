//! Update-game domain contracts and shared data models.

mod models;
mod service;

pub use models::{UpdateGameError, UpdateGameRequest, UpdateGameResult, UpdateGameValidationError};
pub use service::{build_update_command, validate_update_request};
