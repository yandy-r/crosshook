mod common;
mod native;
mod proton_game;
mod proton_resolution;
mod proton_trainer;
mod steam_helpers;
mod trainer_staging;
mod umu;

#[cfg(test)]
mod tests;

pub use common::gamescope_pid_capture_path;
pub use native::build_native_game_command;
pub use proton_game::build_proton_game_command;
pub use proton_trainer::{build_flatpak_steam_trainer_command, build_proton_trainer_command};
pub use steam_helpers::{build_helper_command, build_trainer_command};

pub(crate) use proton_resolution::resolve_launch_proton_path;
pub(crate) use umu::{
    force_no_umu_for_launch_request, proton_path_dirname, resolve_steam_app_id_for_umu,
    should_use_umu,
};
