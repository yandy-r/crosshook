use crosshook_app::services::RecentFilesService;
use crosshook_app::services::{SettingsSaveRequest, SettingsService, SettingsSnapshot};
use crosshook_app::AppError;
use crosshook_core::community::CommunityTapSubscription;
use crosshook_core::discovery::ExternalTrainerSourceSubscription;
use crosshook_core::settings::{
    ConfigHistorySettings, HighContrastPreference, RecentFilesData, ReducedMotionPreference,
    UmuDatabaseLookupPreference, UmuPreference,
};
use serde::{Deserialize, Serialize};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

/// IPC-safe settings DTO returned by `settings_load`.
///
/// The raw SteamGridDB API key is never sent to the frontend. Instead,
/// `has_steamgriddb_api_key` indicates whether a key is currently configured.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
}

impl AppSettingsIpcData {
    fn from_snapshot(snapshot: SettingsSnapshot) -> Self {
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
        }
    }
}

#[tauri::command]
pub async fn settings_load(
    service: State<'_, SettingsService>,
) -> Result<AppSettingsIpcData, String> {
    let snapshot = service.load().await.map_err(map_app_error)?;
    Ok(AppSettingsIpcData::from_snapshot(snapshot))
}

/// Saves the settings form. The SteamGridDB key is never touched; use
/// `settings_save_steamgriddb_key`.
#[tauri::command]
pub async fn settings_save(
    data: SettingsSaveRequest,
    service: State<'_, SettingsService>,
) -> Result<(), String> {
    service
        .save_request(data)
        .await
        .map(drop)
        .map_err(map_app_error)
}

/// Write-only command for updating the SteamGridDB API key.
///
/// Pass `Some(key)` to set the key, or `None` to clear it.
#[tauri::command]
pub async fn settings_save_steamgriddb_key(
    key: Option<String>,
    service: State<'_, SettingsService>,
) -> Result<(), String> {
    service
        .save_steamgriddb_key(key)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn recent_files_load(
    service: State<'_, RecentFilesService>,
) -> Result<RecentFilesData, String> {
    service.load().await.map_err(map_app_error)
}

#[tauri::command]
pub async fn recent_files_save(
    data: RecentFilesData,
    service: State<'_, RecentFilesService>,
) -> Result<(), String> {
    service.save(data).await.map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_core::settings::AppSettingsData;
    use std::path::PathBuf;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = settings_load;
        let _ = settings_save;
        let _ = settings_save_steamgriddb_key;
        let _ = recent_files_load;
        let _ = recent_files_save;
    }

    fn ipc_from(data: AppSettingsData) -> AppSettingsIpcData {
        AppSettingsIpcData::from_snapshot(SettingsSnapshot {
            data,
            resolved_profiles_directory: PathBuf::from("/tmp/profiles"),
            active_profiles_directory: PathBuf::from("/tmp/profiles"),
            profiles_directory_requires_restart: false,
        })
    }

    #[test]
    fn ipc_dto_clears_unknown_keys_and_hides_key() {
        let mut data: AppSettingsData =
            toml::from_str("future = 2026-10-08\nsteamgriddb_api_key = \"secret\"\n").unwrap();
        data.extra
            .insert("future_two".to_string(), toml::Value::Boolean(true));
        let ipc = ipc_from(data);
        assert!(ipc.has_steamgriddb_api_key);
        let json = serde_json::to_string(&ipc).unwrap();
        assert!(!json.contains("future"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn ipc_dto_carries_restart_state_from_snapshot() {
        let ipc = AppSettingsIpcData::from_snapshot(SettingsSnapshot {
            data: AppSettingsData::default(),
            resolved_profiles_directory: PathBuf::from("/tmp/a"),
            active_profiles_directory: PathBuf::from("/tmp/b"),
            profiles_directory_requires_restart: true,
        });
        assert_eq!(ipc.resolved_profiles_directory, "/tmp/a");
        assert_eq!(ipc.active_profiles_directory, "/tmp/b");
        assert!(ipc.profiles_directory_requires_restart);
    }

    #[test]
    fn ipc_dto_reduced_motion_serializes_as_string() {
        let data = AppSettingsData {
            reduced_motion: ReducedMotionPreference::Reduced,
            ..Default::default()
        };
        let ipc = ipc_from(data);
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("\"reduced_motion\":\"reduced\""),
            "serialized IPC DTO must carry the tri-state string form: {json}"
        );
    }

    #[test]
    fn from_parts_surfaces_install_nag_dismissed_at() {
        let data = AppSettingsData {
            install_nag_dismissed_at: Some("2026-04-15T10:00:00Z".to_string()),
            ..Default::default()
        };
        let ipc = ipc_from(data);
        assert_eq!(
            ipc.install_nag_dismissed_at,
            Some("2026-04-15T10:00:00Z".to_string()),
            "from_parts must surface install_nag_dismissed_at to the IPC layer"
        );
    }

    #[test]
    fn from_parts_install_nag_dismissed_at_none_when_unset() {
        let data = AppSettingsData {
            install_nag_dismissed_at: None,
            ..Default::default()
        };
        let ipc = ipc_from(data);
        assert!(
            ipc.install_nag_dismissed_at.is_none(),
            "from_parts must pass through None when field is unset"
        );
    }

    #[test]
    fn settings_ipc_includes_install_nag_field_in_serialized_output() {
        let timestamp = "2026-04-15T10:00:00Z".to_string();
        let data = AppSettingsData {
            install_nag_dismissed_at: Some(timestamp.clone()),
            high_contrast: HighContrastPreference::On,
            ..Default::default()
        };
        let ipc = ipc_from(data);
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("install_nag_dismissed_at"),
            "serialized IPC DTO must contain the install_nag_dismissed_at key"
        );
        assert!(
            json.contains(&timestamp),
            "serialized IPC DTO must contain the timestamp value"
        );
        assert!(
            json.contains("high_contrast"),
            "serialized IPC DTO must include the high_contrast key"
        );
        assert!(
            json.contains("\"high_contrast\":\"on\""),
            "serialized IPC DTO must carry the tri-state string form"
        );
    }

    #[test]
    fn settings_ipc_install_nag_null_serializes_as_json_null() {
        let data = AppSettingsData {
            install_nag_dismissed_at: None,
            ..Default::default()
        };
        let ipc = ipc_from(data);
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("install_nag_dismissed_at"),
            "serialized IPC DTO must always contain the install_nag_dismissed_at key (as null when unset)"
        );
    }

    #[test]
    fn serialization_contains_steam_deck_caveats_dismissed_at() {
        let timestamp = "2026-04-15T12:00:00Z".to_string();
        let data = AppSettingsData {
            steam_deck_caveats_dismissed_at: Some(timestamp.clone()),
            ..Default::default()
        };
        let ipc = ipc_from(data);
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("steam_deck_caveats_dismissed_at"),
            "serialized IPC DTO must contain the steam_deck_caveats_dismissed_at key"
        );
        assert!(
            json.contains(&timestamp),
            "serialized IPC DTO must contain the timestamp value"
        );
    }

    #[test]
    fn settings_ipc_includes_host_tool_dashboard_fields_in_serialized_output() {
        let data = AppSettingsData {
            host_tool_dashboard_dismissed_hints: vec!["gamescope".to_string()],
            host_tool_dashboard_default_category_filter: Some("runtime".to_string()),
            ..Default::default()
        };
        let ipc = ipc_from(data);
        assert_eq!(
            ipc.host_tool_dashboard_dismissed_hints,
            vec!["gamescope".to_string()]
        );
        assert_eq!(
            ipc.host_tool_dashboard_default_category_filter.as_deref(),
            Some("runtime")
        );
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("host_tool_dashboard_dismissed_hints"),
            "serialized IPC DTO must contain host_tool_dashboard_dismissed_hints"
        );
        assert!(
            json.contains("host_tool_dashboard_default_category_filter"),
            "serialized IPC DTO must contain host_tool_dashboard_default_category_filter"
        );
        assert!(json.contains("gamescope"));
        assert!(json.contains("runtime"));
    }
}
