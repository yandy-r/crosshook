use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile::{GamescopeConfig, MangoHudConfig, TrainerLoadingMode};
use crate::settings::UmuPreference;

pub const METHOD_STEAM_APPLAUNCH: &str = "steam_applaunch";
pub const METHOD_PROTON_RUN: &str = "proton_run";
pub const METHOD_NATIVE: &str = "native";

/// Returns `true` if the current process is running inside a gamescope compositor session.
pub fn is_inside_gamescope_session() -> bool {
    std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
}

fn default_network_isolation() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub game_path: String,
    #[serde(default)]
    pub trainer_path: String,
    #[serde(default)]
    pub trainer_host_path: String,
    #[serde(default)]
    pub trainer_loading_mode: TrainerLoadingMode,
    #[serde(default)]
    pub steam: SteamLaunchConfig,
    #[serde(default)]
    pub runtime: RuntimeLaunchConfig,
    #[serde(default)]
    pub optimizations: LaunchOptimizationsRequest,
    #[serde(default)]
    pub launch_trainer_only: bool,
    #[serde(default)]
    pub launch_game_only: bool,
    #[serde(default)]
    pub profile_name: Option<String>,
    #[serde(
        rename = "custom_env_vars",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub custom_env_vars: BTreeMap<String, String>,
    /// User preference for whether to invoke `umu-run` instead of direct Proton.
    /// Defaults to `UmuPreference::Auto`, which prefers umu-run when available and falls back to direct Proton otherwise.
    #[serde(default)]
    pub umu_preference: UmuPreference,
    /// When true, trainer processes are launched in an isolated network namespace
    /// via `unshare --net`.
    #[serde(default = "default_network_isolation")]
    pub network_isolation: bool,
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub gamescope: GamescopeConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_gamescope: Option<GamescopeConfig>,
    #[serde(default, skip_serializing_if = "MangoHudConfig::is_default")]
    pub mangohud: MangoHudConfig,
}

pub type SteamLaunchRequest = LaunchRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamLaunchConfig {
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub compatdata_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeLaunchConfig {
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub working_directory: String,
    /// Steam App ID used as `GAMEID` when launching via `umu-run`.
    /// Falls back to `"umu-0"` when empty (Phase 3).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub steam_app_id: String,
    /// Optional protonfix override. When set, takes precedence over `steam_app_id` for umu GAMEID.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub umu_game_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsRequest {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
}

impl LaunchRequest {
    /// Returns the trainer gamescope config to use at launch time.
    ///
    /// Priority:
    /// 1. Explicit trainer gamescope override when enabled
    /// 2. Auto-generated windowed config derived from the game gamescope config
    /// 3. Default disabled config when the game gamescope config is disabled
    pub fn resolved_trainer_gamescope(&self) -> GamescopeConfig {
        super::super::resolve_trainer_gamescope(&self.gamescope, self.trainer_gamescope.as_ref())
    }

    pub fn effective_gamescope_config(&self) -> GamescopeConfig {
        if self.launch_trainer_only {
            self.resolved_trainer_gamescope()
        } else {
            self.gamescope.clone()
        }
    }

    pub fn resolved_method(&self) -> &str {
        match self.method.trim() {
            METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
            METHOD_PROTON_RUN => METHOD_PROTON_RUN,
            METHOD_NATIVE => METHOD_NATIVE,
            _ if !self.steam.app_id.trim().is_empty() => METHOD_STEAM_APPLAUNCH,
            _ if looks_like_windows_executable(&self.game_path) => METHOD_PROTON_RUN,
            _ => METHOD_NATIVE,
        }
    }

    pub fn game_executable_name(&self) -> String {
        let trimmed_path = self.game_path.trim();

        if trimmed_path.is_empty() {
            return String::new();
        }

        let separator_index = trimmed_path
            .char_indices()
            .rev()
            .find_map(|(index, character)| matches!(character, '/' | '\\').then_some(index));

        match separator_index {
            Some(index) if index + 1 < trimmed_path.len() => trimmed_path[index + 1..].to_string(),
            Some(_) => String::new(),
            None => trimmed_path.to_string(),
        }
    }

    pub fn log_target_slug(&self) -> String {
        let game_executable_name = self.game_executable_name();
        let source = match self.resolved_method() {
            METHOD_STEAM_APPLAUNCH => self.steam.app_id.trim(),
            _ => game_executable_name.trim(),
        };

        let fallback = match self.resolved_method() {
            METHOD_STEAM_APPLAUNCH => "steam",
            METHOD_PROTON_RUN => "proton",
            METHOD_NATIVE => "native",
            _ => "launch",
        };

        let candidate = if source.is_empty() { fallback } else { source };
        let slug = candidate
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();

        let trimmed = slug.trim_matches('-');
        if trimmed.is_empty() {
            fallback.to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn should_copy_trainer_to_prefix(&self) -> bool {
        self.trainer_loading_mode == TrainerLoadingMode::CopyToPrefix
    }
}

pub(super) fn looks_like_windows_executable(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".exe")
}
