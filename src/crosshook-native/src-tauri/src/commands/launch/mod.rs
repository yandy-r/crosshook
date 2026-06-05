//! Launch Tauri commands, split into focused submodules.
//!
//! - `shared`      — shared DTOs, constants, and log-relay helpers
//! - `queries`     — lightweight read-only launch commands
//! - `warnings`    — non-blocking advisory collectors
//! - `portal`      — Flatpak GameMode portal registration helpers
//! - `diagnostics` — launch-log diagnostic helpers
//! - `streaming`   — child/log streaming orchestration
//! - `execution`   — launch command entrypoints and watchdog spawning

mod diagnostics;
mod execution;
mod portal;
mod queries;
mod shared;
mod streaming;
#[cfg(test)]
mod tests;
mod warnings;

pub use execution::{launch_game, launch_trainer};
pub use queries::{
    build_steam_launch_options_command, check_game_running, check_gamescope_session,
    launch_platform_status, list_launch_history_for_profile, list_running_profiles, preview_launch,
    validate_launch,
};

// Re-export Tauri command macros so `generate_handler!` can resolve `commands::launch::<name>`.
pub use execution::__cmd__launch_game;
pub use execution::__cmd__launch_trainer;
pub use execution::__tauri_command_name_launch_game;
pub use execution::__tauri_command_name_launch_trainer;
pub use queries::__cmd__build_steam_launch_options_command;
pub use queries::__cmd__check_game_running;
pub use queries::__cmd__check_gamescope_session;
pub use queries::__cmd__launch_platform_status;
pub use queries::__cmd__list_launch_history_for_profile;
pub use queries::__cmd__list_running_profiles;
pub use queries::__cmd__preview_launch;
pub use queries::__cmd__validate_launch;
pub use queries::__tauri_command_name_build_steam_launch_options_command;
pub use queries::__tauri_command_name_check_game_running;
pub use queries::__tauri_command_name_check_gamescope_session;
pub use queries::__tauri_command_name_launch_platform_status;
pub use queries::__tauri_command_name_list_launch_history_for_profile;
pub use queries::__tauri_command_name_list_running_profiles;
pub use queries::__tauri_command_name_preview_launch;
pub use queries::__tauri_command_name_validate_launch;
