//! Install-game domain contracts and shared data models.

mod discovery;
mod models;
mod service;

pub use discovery::discover_game_executable_candidates;
pub use models::{
    InstallGameError, InstallGameRequest, InstallGameResult, InstallGameValidationError,
};
pub use service::{
    install_default_prefix_path, install_game, resolve_default_prefix_path,
    validate_install_request,
};
