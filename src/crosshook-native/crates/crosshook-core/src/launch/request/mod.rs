mod error;
mod error_text;
mod issues;
mod models;
mod path_probe;
mod validation;

#[cfg(test)]
mod tests;

pub use error::ValidationError;
pub use issues::{LaunchValidationIssue, ValidationSeverity};
pub use models::{
    is_inside_gamescope_session, LaunchOptimizationsRequest, LaunchRequest, RuntimeLaunchConfig,
    SteamLaunchConfig, SteamLaunchRequest, METHOD_NATIVE, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
pub use validation::{validate, validate_all};

pub(crate) use path_probe::{path_exists_visible_or_host, path_is_executable_visible_or_host};
