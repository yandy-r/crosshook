//! Settings service: typed, atomic mutation of `settings.toml`.

use std::path::{Path, PathBuf};

use crosshook_core::community::CommunityTapSubscription;
use crosshook_core::discovery::ExternalTrainerSourceSubscription;
use crosshook_core::settings::{
    clamp_config_history_max_revisions, clamp_recent_files_limit,
    resolve_profiles_directory_from_config, AppSettingsData, ConfigHistorySettings,
    HighContrastPreference, ReducedMotionPreference, SettingsStore, UmuDatabaseLookupPreference,
    UmuPreference,
};
use serde::{Deserialize, Deserializer, Serialize};

use super::run_blocking;

use crate::{AppError, AppErrorKind};

mod ui;
mod ui_writer;

pub use ui::UiSettingsService;

/// Log filter used when the configured filter is empty or whitespace.
const DEFAULT_LOG_FILTER: &str = "info";

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ui_tests;

#[cfg(test)]
mod ui_writer_tests;

/// Settings read together with the profiles-directory restart state.
#[derive(Debug, Clone)]
pub struct SettingsSnapshot {
    /// Settings as stored on disk (unknown keys retained in `extra`).
    pub data: AppSettingsData,
    /// Profiles directory resolved from the current settings (tilde-expanded).
    pub resolved_profiles_directory: PathBuf,
    /// Profiles directory the running process actually uses.
    pub active_profiles_directory: PathBuf,
    /// `true` when the resolved and active directories differ, so a change
    /// only takes effect after restart.
    pub profiles_directory_requires_restart: bool,
}

/// Save request for the settings form.
///
/// Excludes the SteamGridDB API key — use
/// [`SettingsService::save_steamgriddb_key`] to update the key. This prevents
/// an accidental frontend round-trip from clearing the stored key. Optional
/// fields preserve the stored value when absent; `Option<Option<_>>` fields
/// additionally clear the value on an explicit `null`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub struct SettingsSaveRequest {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub default_proton_path: String,
    pub default_launch_method: String,
    pub default_bundled_optimization_preset_id: String,
    pub default_trainer_loading_mode: String,
    pub log_filter: String,
    pub console_drawer_collapsed_default: bool,
    pub recent_files_limit: u32,
    pub profiles_directory: String,
    pub protontricks_binary_path: String,
    pub auto_install_prefix_deps: bool,
    pub discovery_enabled: bool,
    pub external_trainer_sources: Option<Vec<ExternalTrainerSourceSubscription>>,
    #[serde(default)]
    pub high_contrast: Option<HighContrastPreference>,
    #[serde(default)]
    pub reduced_motion: Option<ReducedMotionPreference>,
    #[serde(default)]
    pub protonup_auto_suggest: Option<bool>,
    #[serde(default)]
    pub protonup_binary_path: Option<String>,
    #[serde(default)]
    pub umu_preference: Option<UmuPreference>,
    #[serde(default)]
    pub umu_database_lookup: Option<UmuDatabaseLookupPreference>,
    #[serde(default)]
    pub host_tool_dashboard_dismissed_hints: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub host_tool_dashboard_default_category_filter: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub install_nag_dismissed_at: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub steam_deck_caveats_dismissed_at: Option<Option<String>>,
    #[serde(default)]
    pub protonup_default_provider: Option<String>,
    #[serde(default)]
    pub protonup_default_install_root: Option<String>,
    #[serde(default)]
    pub protonup_include_prereleases: Option<bool>,
    #[serde(default)]
    pub config_history: Option<ConfigHistorySettings>,
}

/// Deserializes a key that is present (value or `null`) as `Some(_)`, so an
/// explicit `null` becomes `Some(None)` ("clear"). Paired with
/// `#[serde(default)]`, an absent key stays `None` ("preserve").
fn deserialize_present<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Merges a [`SettingsSaveRequest`] over `current` settings.
///
/// The SteamGridDB key and all unknown on-disk keys are preserved from
/// `current`; unknown keys carried by the request are discarded. The result is
/// normalized (limits clamped, empty log filter defaulted).
pub fn merge_save_request(data: SettingsSaveRequest, current: AppSettingsData) -> AppSettingsData {
    let disk_extras = current.clone();
    let mut merged = AppSettingsData {
        extra: Default::default(),
        auto_load_last_profile: data.auto_load_last_profile,
        last_used_profile: data.last_used_profile,
        community_taps: data.community_taps,
        onboarding_completed: data.onboarding_completed,
        offline_mode: data.offline_mode,
        steamgriddb_api_key: current.steamgriddb_api_key,
        default_proton_path: data.default_proton_path,
        default_launch_method: data.default_launch_method,
        default_bundled_optimization_preset_id: data.default_bundled_optimization_preset_id,
        default_trainer_loading_mode: data.default_trainer_loading_mode,
        log_filter: data.log_filter,
        console_drawer_collapsed_default: data.console_drawer_collapsed_default,
        recent_files_limit: data.recent_files_limit,
        profiles_directory: data.profiles_directory,
        protontricks_binary_path: data.protontricks_binary_path,
        auto_install_prefix_deps: data.auto_install_prefix_deps,
        discovery_enabled: data.discovery_enabled,
        // Preserve current sources only when the field is omitted.
        // An explicit empty list means "save no sources".
        external_trainer_sources: data
            .external_trainer_sources
            .unwrap_or(current.external_trainer_sources),
        high_contrast: data.high_contrast.unwrap_or(current.high_contrast),
        reduced_motion: data.reduced_motion.unwrap_or(current.reduced_motion),
        protonup_auto_suggest: data
            .protonup_auto_suggest
            .unwrap_or(current.protonup_auto_suggest),
        protonup_binary_path: data
            .protonup_binary_path
            .unwrap_or(current.protonup_binary_path),
        umu_preference: data.umu_preference.unwrap_or(current.umu_preference),
        umu_database_lookup: data
            .umu_database_lookup
            .unwrap_or(current.umu_database_lookup),
        host_tool_dashboard_dismissed_hints: data
            .host_tool_dashboard_dismissed_hints
            .unwrap_or(current.host_tool_dashboard_dismissed_hints),
        host_tool_dashboard_default_category_filter: data
            .host_tool_dashboard_default_category_filter
            .unwrap_or(current.host_tool_dashboard_default_category_filter),
        // Absent field preserves current value; explicit null clears the timestamp.
        install_nag_dismissed_at: data
            .install_nag_dismissed_at
            .unwrap_or(current.install_nag_dismissed_at),
        steam_deck_caveats_dismissed_at: data
            .steam_deck_caveats_dismissed_at
            .unwrap_or(current.steam_deck_caveats_dismissed_at),
        protonup_default_provider: data
            .protonup_default_provider
            .unwrap_or(current.protonup_default_provider),
        protonup_default_install_root: data
            .protonup_default_install_root
            .unwrap_or(current.protonup_default_install_root),
        protonup_include_prereleases: data
            .protonup_include_prereleases
            .unwrap_or(current.protonup_include_prereleases),
        config_history: data.config_history.unwrap_or(current.config_history),
        // The form never carries `[ui]`; keep the stored table (D4).
        ui: current.ui,
    };
    merged.preserve_extra_from(&disk_extras);
    normalize(&mut merged);
    merged
}

/// Clamps bounded limits and defaults an empty log filter.
fn normalize(settings: &mut AppSettingsData) {
    settings.recent_files_limit = clamp_recent_files_limit(settings.recent_files_limit);
    settings.config_history.max_revisions =
        clamp_config_history_max_revisions(settings.config_history.max_revisions);
    settings.ui.normalize();
    let log_filter = settings.log_filter.trim();
    settings.log_filter = if log_filter.is_empty() {
        DEFAULT_LOG_FILTER.to_string()
    } else {
        log_filter.to_string()
    };
}

/// `true` when `resolved` and `active` name different directories. Compares
/// canonical paths when both exist, otherwise falls back to lexical equality.
fn paths_need_restart_for_profiles(resolved: &Path, active: &Path) -> bool {
    match (resolved.canonicalize(), active.canonicalize()) {
        (Ok(r), Ok(a)) => r != a,
        _ => resolved != active,
    }
}

/// Settings service over a [`SettingsStore`]. Cheap to clone.
#[derive(Debug, Clone)]
pub struct SettingsService {
    store: SettingsStore,
    active_profiles_dir: PathBuf,
}

impl SettingsService {
    /// Creates a service; `active_profiles_dir` is the profiles directory the
    /// running process uses (for restart detection).
    pub fn new(store: SettingsStore, active_profiles_dir: PathBuf) -> Self {
        Self {
            store,
            active_profiles_dir,
        }
    }

    /// Loads settings and resolves the profiles-directory restart state.
    pub async fn load(&self) -> Result<SettingsSnapshot, AppError> {
        let store = self.store.clone();
        let active = self.active_profiles_dir.clone();
        run_blocking(move || {
            let data = store.load()?;
            let resolved = resolve_profiles_directory_from_config(&data, &store.base_path)
                .map_err(|message| {
                    AppError::new(
                        AppErrorKind::Validation {
                            field: "profiles_directory",
                        },
                        message,
                    )
                })?;
            let profiles_directory_requires_restart =
                paths_need_restart_for_profiles(&resolved, &active);
            Ok(SettingsSnapshot {
                data,
                resolved_profiles_directory: resolved,
                active_profiles_directory: active,
                profiles_directory_requires_restart,
            })
        })
        .await
    }

    /// Atomically loads, mutates, normalizes, and saves settings. Unknown
    /// on-disk keys are preserved. Returns the saved settings.
    pub async fn update<F>(&self, mutate: F) -> Result<AppSettingsData, AppError>
    where
        F: FnOnce(&mut AppSettingsData) + Send + 'static,
    {
        let store = self.store.clone();
        run_blocking(move || {
            let saved = store
                .update(|settings| {
                    mutate(settings);
                    normalize(settings);
                    Ok::<_, std::convert::Infallible>(settings.clone())
                })?
                .unwrap_or_else(|never| match never {});
            Ok(saved)
        })
        .await
    }

    /// Merges `request` over the stored settings and saves atomically.
    pub async fn save_request(
        &self,
        request: SettingsSaveRequest,
    ) -> Result<AppSettingsData, AppError> {
        self.update(move |settings| {
            *settings = merge_save_request(request, settings.clone());
        })
        .await
    }

    /// Sets (trimmed) or clears the SteamGridDB API key. An empty or
    /// whitespace key clears it. Only the key changes.
    pub async fn save_steamgriddb_key(&self, key: Option<String>) -> Result<(), AppError> {
        let key = key
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty());
        let store = self.store.clone();
        run_blocking(move || {
            store
                .update(|settings| {
                    settings.steamgriddb_api_key = key;
                    Ok::<_, std::convert::Infallible>(())
                })?
                .unwrap_or_else(|never| match never {});
            Ok(())
        })
        .await
    }
}
