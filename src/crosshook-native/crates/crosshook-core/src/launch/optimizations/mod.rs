//! Launch optimization directives and Steam launch options construction.

mod command_check;
mod directives;
mod gamemode;
mod steam_options;

// Re-export public API
pub use command_check::is_command_available;
pub use directives::{
    is_known_launch_optimization_id, resolve_launch_directives,
    resolve_launch_directives_for_method, LaunchDirectives,
};
pub use gamemode::{should_register_gamemode_portal, USE_GAMEMODE_OPTIMIZATION_ID};
pub use steam_options::{build_steam_launch_options_command, escape_steam_token};

// Re-export test helpers
#[cfg(test)]
pub(crate) use command_check::{resolve_umu_run_path_for_test, swap_test_command_search_path};
