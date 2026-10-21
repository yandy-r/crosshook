//! Canonical `GameProfile` → `LaunchRequest` builder shared by every frontend.

use std::fmt;

use crate::profile::GameProfile;
use crate::settings::UmuPreference;

use super::models::{
    LaunchCommandArgumentsRequest, LaunchOptimizationsRequest, LaunchRequest, RuntimeLaunchConfig,
    SteamLaunchConfig, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};

/// Reasons a profile cannot be turned into a [`LaunchRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRequestError {
    /// `game.executable_path` is blank.
    MissingGamePath,
    /// The supplied launch method is not one of the supported methods.
    UnsupportedMethod(String),
}

impl fmt::Display for LaunchRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGamePath => f.write_str("profile has no game executable path"),
            Self::UnsupportedMethod(method) => {
                write!(f, "unsupported launch method \"{method}\"")
            }
        }
    }
}

impl std::error::Error for LaunchRequestError {}

impl LaunchRequest {
    /// Builds the launch request for `profile` executed with `method`.
    ///
    /// The result is flag-neutral (`launch_game_only` and `launch_trainer_only`
    /// are both `false`); callers set the phase flag. `umu_default` applies only
    /// when the profile carries no `runtime.umu_preference` override.
    pub fn from_profile(
        profile: &GameProfile,
        method: &str,
        steam_client_path: &str,
        profile_name: &str,
        umu_default: UmuPreference,
    ) -> Result<Self, LaunchRequestError> {
        let method = method.trim();
        if !matches!(
            method,
            METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE
        ) {
            return Err(LaunchRequestError::UnsupportedMethod(method.to_string()));
        }
        if profile.game.executable_path.trim().is_empty() {
            return Err(LaunchRequestError::MissingGamePath);
        }

        let launch = &profile.launch;
        let runtime = &profile.runtime;
        Ok(Self {
            method: method.to_string(),
            game_path: profile.game.executable_path.clone(),
            trainer_path: profile.trainer.path.clone(),
            trainer_host_path: profile.trainer.path.clone(),
            trainer_loading_mode: profile.trainer.loading_mode,
            steam: SteamLaunchConfig {
                app_id: profile.steam.app_id.clone(),
                compatdata_path: profile.steam.compatdata_path.clone(),
                proton_path: profile.steam.proton_path.clone(),
                steam_client_install_path: steam_client_path.to_string(),
            },
            runtime: RuntimeLaunchConfig {
                prefix_path: runtime.prefix_path.clone(),
                proton_path: runtime.proton_path.clone(),
                working_directory: runtime.working_directory.clone(),
                steam_app_id: runtime.steam_app_id.clone(),
                umu_game_id: runtime.umu_game_id.clone(),
                umu_store: runtime.umu_store.clone(),
                umu_codename: runtime.umu_codename.clone(),
            },
            optimizations: LaunchOptimizationsRequest {
                enabled_option_ids: launch.optimizations.enabled_option_ids.clone(),
            },
            command_arguments: LaunchCommandArgumentsRequest {
                enabled_argument_ids: launch.command_arguments.enabled_argument_ids.clone(),
                custom_args: launch.command_arguments.custom_args.clone(),
            },
            launch_trainer_only: false,
            launch_game_only: false,
            profile_name: (!profile_name.trim().is_empty()).then(|| profile_name.to_string()),
            custom_env_vars: launch.custom_env_vars.clone(),
            umu_preference: runtime.umu_preference.unwrap_or(umu_default),
            resolved_umu_game_id: None,
            network_isolation: launch.network_isolation,
            gamescope: launch.gamescope.clone(),
            trainer_gamescope: (!launch.trainer_gamescope.is_default())
                .then(|| launch.trainer_gamescope.clone()),
            mangohud: launch.mangohud.clone(),
            pre_launch_hooks: profile.pre_launch_hooks.clone(),
            post_exit_hooks: profile.post_exit_hooks.clone(),
        })
    }
}
