use crosshook_core::community::CommunityTapSubscription;
use crosshook_core::settings::{
    AppSettingsData, RecentFilesData, RecentFilesStore, RecentFilesStoreError, SettingsStore,
    SettingsStoreError,
};
use serde::{Deserialize, Serialize};
use tauri::State;

fn map_settings_error(error: SettingsStoreError) -> String {
    error.to_string()
}

fn map_recent_files_error(error: RecentFilesStoreError) -> String {
    error.to_string()
}

/// IPC-safe settings DTO returned by `settings_load`.
///
/// The raw SteamGridDB API key is never sent to the frontend. Instead,
/// `has_steamgriddb_api_key` indicates whether a key is currently configured.
#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettingsIpcData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub has_steamgriddb_api_key: bool,
}

impl From<AppSettingsData> for AppSettingsIpcData {
    fn from(data: AppSettingsData) -> Self {
        Self {
            auto_load_last_profile: data.auto_load_last_profile,
            last_used_profile: data.last_used_profile,
            community_taps: data.community_taps,
            onboarding_completed: data.onboarding_completed,
            offline_mode: data.offline_mode,
            has_steamgriddb_api_key: data
                .steamgriddb_api_key
                .as_deref()
                .map(|k| !k.is_empty())
                .unwrap_or(false),
        }
    }
}

/// IPC request DTO for `settings_save`.
///
/// Excludes the SteamGridDB API key — use `settings_save_steamgriddb_key` to
/// update the key. This prevents an accidental frontend round-trip from
/// clearing the stored key.
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsSaveRequest {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
}

#[tauri::command]
pub fn settings_load(store: State<'_, SettingsStore>) -> Result<AppSettingsIpcData, String> {
    store
        .load()
        .map(AppSettingsIpcData::from)
        .map_err(map_settings_error)
}

#[tauri::command]
pub fn settings_save(
    data: SettingsSaveRequest,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    // Load the current settings so the API key is preserved across saves that
    // do not touch it.
    let current = store.load().map_err(map_settings_error)?;
    let merged = AppSettingsData {
        auto_load_last_profile: data.auto_load_last_profile,
        last_used_profile: data.last_used_profile,
        community_taps: data.community_taps,
        onboarding_completed: data.onboarding_completed,
        offline_mode: data.offline_mode,
        steamgriddb_api_key: current.steamgriddb_api_key,
    };
    store.save(&merged).map_err(map_settings_error)
}

/// Write-only command for updating the SteamGridDB API key.
///
/// Pass `Some(key)` to set the key, or `None` to clear it.
#[tauri::command]
pub fn settings_save_steamgriddb_key(
    key: Option<String>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let mut current = store.load().map_err(map_settings_error)?;
    current.steamgriddb_api_key = key.and_then(|k| {
        let trimmed = k.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    store.save(&current).map_err(map_settings_error)
}

#[tauri::command]
pub fn recent_files_load(store: State<'_, RecentFilesStore>) -> Result<RecentFilesData, String> {
    store.load().map_err(map_recent_files_error)
}

#[tauri::command]
pub fn recent_files_save(
    data: RecentFilesData,
    store: State<'_, RecentFilesStore>,
) -> Result<(), String> {
    store.save(&data).map_err(map_recent_files_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = settings_load as fn(State<'_, SettingsStore>) -> Result<AppSettingsIpcData, String>;
        let _ = settings_save
            as fn(SettingsSaveRequest, State<'_, SettingsStore>) -> Result<(), String>;
        let _ = settings_save_steamgriddb_key
            as fn(Option<String>, State<'_, SettingsStore>) -> Result<(), String>;
        let _ =
            recent_files_load as fn(State<'_, RecentFilesStore>) -> Result<RecentFilesData, String>;
        let _ = recent_files_save
            as fn(RecentFilesData, State<'_, RecentFilesStore>) -> Result<(), String>;
    }
}
