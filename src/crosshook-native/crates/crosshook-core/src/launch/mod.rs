//! Launch orchestration primitives.

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
pub use runtime_helpers::resolve_umu_run_path;
