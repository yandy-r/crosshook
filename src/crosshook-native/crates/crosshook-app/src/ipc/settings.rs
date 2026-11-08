//! `settings_load` result: the settings snapshot minus secrets and unknown keys.

use crosshook_core::community::CommunityTapSubscription;
use crosshook_core::discovery::ExternalTrainerSourceSubscription;
use crosshook_core::settings::{
    ConfigHistorySettings, HighContrastPreference, ReducedMotionPreference, UiSettings,
    UmuDatabaseLookupPreference, UmuPreference,
};
use serde::{Deserialize, Serialize};

use crate::services::{SettingsService, SettingsSnapshot, UiSettingsService};
use crate::AppError;

/// IPC-safe settings DTO returned by `settings_load`.
///
/// The raw SteamGridDB API key is never sent to the frontend. Instead,
/// `has_steamgriddb_api_key` indicates whether a key is currently configured.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
// Field-for-field mirror of `AppSettingsData`, whose fields carry the docs.
#[allow(missing_docs)]
pub struct AppSettingsIpcData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub high_contrast: HighContrastPreference,
    pub reduced_motion: ReducedMotionPreference,
    pub has_steamgriddb_api_key: bool,
    pub default_proton_path: String,
    pub default_launch_method: String,
    pub default_bundled_optimization_preset_id: String,
    pub default_trainer_loading_mode: String,
    pub log_filter: String,
    pub console_drawer_collapsed_default: bool,
    pub recent_files_limit: u32,
    pub profiles_directory: String,
    /// Path from current `settings.toml` (after expand); may differ from active until restart.
    pub resolved_profiles_directory: String,
    pub active_profiles_directory: String,
    pub profiles_directory_requires_restart: bool,
    pub protontricks_binary_path: String,
    pub auto_install_prefix_deps: bool,
    pub discovery_enabled: bool,
    pub external_trainer_sources: Vec<ExternalTrainerSourceSubscription>,
    pub protonup_auto_suggest: bool,
    pub protonup_binary_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protonup_default_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protonup_default_install_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protonup_include_prereleases: Option<bool>,
    pub umu_preference: UmuPreference,
    pub umu_database_lookup: UmuDatabaseLookupPreference,
    /// RFC 3339 timestamp of when the user dismissed the umu install nag; `None` = not dismissed.
    pub install_nag_dismissed_at: Option<String>,
    /// RFC 3339 timestamp of when the user dismissed the Steam Deck caveats; `None` = not dismissed.
    pub steam_deck_caveats_dismissed_at: Option<String>,
    /// Capability ids whose inline install-hint banners were dismissed in gating rows (TOML preference).
    pub host_tool_dashboard_dismissed_hints: Vec<String>,
    /// Default category filter for the Host Tools dashboard (`None` = all).
    pub host_tool_dashboard_default_category_filter: Option<String>,
    pub config_history: ConfigHistorySettings,
    /// Typed `[ui]` preferences (unknown keys are never sent to the frontend).
    pub ui: UiSettings,
}

impl AppSettingsIpcData {
    /// Builds the DTO from a settings snapshot: unknown keys are dropped and
    /// the SteamGridDB key is reduced to its presence flag.
    pub fn from_snapshot(snapshot: SettingsSnapshot) -> Self {
        let SettingsSnapshot {
            mut data,
            resolved_profiles_directory,
            active_profiles_directory,
            profiles_directory_requires_restart,
        } = snapshot;
        data.clear_extra();
        let resolved_profiles_directory = resolved_profiles_directory.display().to_string();
        let active_profiles_directory = active_profiles_directory.display().to_string();
        Self {
            auto_load_last_profile: data.auto_load_last_profile,
            last_used_profile: data.last_used_profile,
            community_taps: data.community_taps,
            onboarding_completed: data.onboarding_completed,
            offline_mode: data.offline_mode,
            high_contrast: data.high_contrast,
            reduced_motion: data.reduced_motion,
            has_steamgriddb_api_key: data
                .steamgriddb_api_key
                .as_deref()
                .map(|k| !k.is_empty())
                .unwrap_or(false),
            default_proton_path: data.default_proton_path,
            default_launch_method: data.default_launch_method,
            default_bundled_optimization_preset_id: data.default_bundled_optimization_preset_id,
            default_trainer_loading_mode: data.default_trainer_loading_mode,
            log_filter: data.log_filter,
            console_drawer_collapsed_default: data.console_drawer_collapsed_default,
            recent_files_limit: data.recent_files_limit,
            profiles_directory: data.profiles_directory,
            resolved_profiles_directory,
            active_profiles_directory,
            profiles_directory_requires_restart,
            protontricks_binary_path: data.protontricks_binary_path,
            auto_install_prefix_deps: data.auto_install_prefix_deps,
            discovery_enabled: data.discovery_enabled,
            external_trainer_sources: data.external_trainer_sources,
            protonup_auto_suggest: data.protonup_auto_suggest,
            protonup_binary_path: data.protonup_binary_path,
            protonup_default_provider: Some(data.protonup_default_provider),
            protonup_default_install_root: Some(data.protonup_default_install_root),
            protonup_include_prereleases: Some(data.protonup_include_prereleases),
            umu_preference: data.umu_preference,
            umu_database_lookup: data.umu_database_lookup,
            install_nag_dismissed_at: data.install_nag_dismissed_at,
            steam_deck_caveats_dismissed_at: data.steam_deck_caveats_dismissed_at,
            host_tool_dashboard_dismissed_hints: data.host_tool_dashboard_dismissed_hints,
            host_tool_dashboard_default_category_filter: data
                .host_tool_dashboard_default_category_filter,
            config_history: data.config_history,
            ui: data.ui,
        }
    }
}

/// Loads the settings for `settings_load`. The in-memory `[ui]` snapshot
/// replaces the stored one because it may be ahead of disk while a throttled
/// write is pending.
///
/// # Errors
/// Settings read failure, mapped to its sanitized kind.
pub async fn load(
    settings: &SettingsService,
    ui: &UiSettingsService,
) -> Result<AppSettingsIpcData, AppError> {
    let mut snapshot = settings.load().await?;
    snapshot.data.ui = ui.get();
    Ok(AppSettingsIpcData::from_snapshot(snapshot))
}

#[cfg(test)]
mod tests;
