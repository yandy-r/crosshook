//! Launch orchestration primitives.

use crate::profile::GamescopeConfig;

pub mod catalog;
pub mod diagnostics;
pub mod env;
pub mod mangohud_presets;
pub mod optimizations;
pub mod preview;
pub mod request;
pub mod runtime_helpers;
pub mod script_runner;
#[cfg(test)]
pub(crate) mod test_support;
pub mod trainer_hash;
pub mod watchdog;

pub use catalog::{
    global_catalog, initialize_catalog, load_catalog, OptimizationCatalog, OptimizationEntry,
};
pub use diagnostics::{analyze, should_surface_report, DiagnosticReport};
pub use env::{
    BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS, PASSTHROUGH_DISPLAY_VARS, REQUIRED_PROTON_VARS,
    WINE_ENV_VARS_TO_CLEAR,
};
pub use optimizations::{
    build_steam_launch_options_command, escape_steam_token, is_known_launch_optimization_id,
    resolve_launch_directives, resolve_launch_directives_for_method, LaunchDirectives,
};
pub use preview::{build_launch_preview, LaunchPreview};
pub use request::{
    is_inside_gamescope_session, validate, validate_all, LaunchRequest, LaunchValidationIssue,
    RuntimeLaunchConfig, SteamLaunchConfig, SteamLaunchRequest, ValidationError,
    ValidationSeverity, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
pub use runtime_helpers::{launch_platform_capabilities, LaunchPlatformCapabilities};
pub use trainer_hash::{
    collect_trainer_hash_launch_warnings, launch_issues_from_trainer_hash_outcome,
};
pub use watchdog::{gamescope_watchdog, is_process_running};

/// Resolves the trainer gamescope config from a game config and an optional trainer override.
///
/// Priority:
/// 1. `trainer_override` when `Some` and `enabled` — returned as-is (cloned).
/// 2. `game` when `enabled` — cloned with `fullscreen` and `borderless` forced to `false`
///    so the trainer window is windowed inside the game's compositor session.
/// 3. [`GamescopeConfig::default`] when the game config is disabled.
pub(crate) fn resolve_trainer_gamescope(
    game: &GamescopeConfig,
    trainer_override: Option<&GamescopeConfig>,
) -> GamescopeConfig {
    if let Some(trainer) = trainer_override.filter(|c| c.enabled) {
        return trainer.clone();
    }

    if game.enabled {
        let mut derived = game.clone();
        derived.fullscreen = false;
        derived.borderless = false;
        return derived;
    }

    GamescopeConfig::default()
}
