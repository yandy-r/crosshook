//! Launch orchestration primitives.

pub mod env;
pub mod optimizations;
pub mod request;
pub mod runtime_helpers;
pub mod script_runner;
#[cfg(test)]
pub(crate) mod test_support;

pub use env::{
    LAUNCH_OPTIMIZATION_ENV_VARS, PASSTHROUGH_DISPLAY_VARS, REQUIRED_PROTON_VARS,
    WINE_ENV_VARS_TO_CLEAR,
};
pub use optimizations::{resolve_launch_directives, LaunchDirectives};
pub use request::{
    validate, LaunchRequest, RuntimeLaunchConfig, SteamLaunchConfig, SteamLaunchRequest,
    ValidationError, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
