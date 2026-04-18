use serde::{Deserialize, Serialize};

use crate::settings::UmuPreference;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeSection {
    #[serde(rename = "prefix_path", default)]
    pub prefix_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
    #[serde(rename = "working_directory", default)]
    pub working_directory: String,
    /// Optional Steam App ID for media/metadata lookup only.
    /// Does NOT affect launch behavior.
    #[serde(
        rename = "steam_app_id",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub steam_app_id: String,
    /// Optional `GAMEID` override for umu-run launches.
    /// Takes precedence over `steam_app_id` when set. Empty → falls back to Steam App ID or "umu-0".
    #[serde(
        rename = "umu_game_id",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub umu_game_id: String,
    /// Optional per-profile umu preference override.
    /// `None` (TOML key absent) → inherit `AppSettingsData.umu_preference` global default.
    /// `Some(x)` → use `x` regardless of global default.
    #[serde(
        rename = "umu_preference",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub umu_preference: Option<UmuPreference>,
}

impl RuntimeSection {
    pub fn is_empty(&self) -> bool {
        self.prefix_path.trim().is_empty()
            && self.proton_path.trim().is_empty()
            && self.working_directory.trim().is_empty()
            && self.steam_app_id.trim().is_empty()
            && self.umu_game_id.trim().is_empty()
            && self.umu_preference.is_none()
    }
}
