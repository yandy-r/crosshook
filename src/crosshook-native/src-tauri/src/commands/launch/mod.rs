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
    launch_platform_status, preview_launch, validate_launch,
};

pub use execution::{__cmd__launch_game, __cmd__launch_trainer};
pub use queries::{
    __cmd__build_steam_launch_options_command, __cmd__check_game_running,
    __cmd__check_gamescope_session, __cmd__launch_platform_status, __cmd__preview_launch,
    __cmd__validate_launch,
};
