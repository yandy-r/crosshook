use crosshook_core::community::CommunityTapSubscription;
use crosshook_core::discovery::ExternalTrainerSourceSubscription;
use crosshook_core::settings::{
    clamp_recent_files_limit, resolve_profiles_directory_from_config, AppSettingsData,
    RecentFilesData, RecentFilesStore, RecentFilesStoreError, SettingsStore, SettingsStoreError,
    UmuPreference,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
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
#[serde(rename_all = "snake_case")]
pub struct AppSettingsIpcData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
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
    pub umu_preference: UmuPreference,
    /// RFC 3339 timestamp of when the user dismissed the umu install nag; `None` = not dismissed.
    pub install_nag_dismissed_at: Option<String>,
}

impl AppSettingsIpcData {
    fn from_parts(data: AppSettingsData, resolved_profiles: &Path, active_profiles: &Path) -> Self {
        let resolved_profiles_directory = resolved_profiles.display().to_string();
        let active_profiles_directory = active_profiles.display().to_string();
        let profiles_directory_requires_restart =
            paths_need_restart_for_profiles(resolved_profiles, active_profiles);
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
            umu_preference: data.umu_preference,
            install_nag_dismissed_at: data.install_nag_dismissed_at,
        }
    }
}

fn paths_need_restart_for_profiles(resolved: &Path, active: &Path) -> bool {
    match (resolved.canonicalize(), active.canonicalize()) {
        (Ok(r), Ok(a)) => r != a,
        _ => resolved != active,
    }
}

/// IPC request DTO for `settings_save`.
///
/// Excludes the SteamGridDB API key — use `settings_save_steamgriddb_key` to
/// update the key. This prevents an accidental frontend round-trip from
/// clearing the stored key.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub protonup_auto_suggest: Option<bool>,
    #[serde(default)]
    pub protonup_binary_path: Option<String>,
    #[serde(default)]
    pub umu_preference: Option<UmuPreference>,
    #[serde(default)]
    pub install_nag_dismissed_at: Option<Option<String>>,
}

fn merge_settings_from_request(
    data: SettingsSaveRequest,
    current: AppSettingsData,
) -> AppSettingsData {
    let recent_files_limit = clamp_recent_files_limit(data.recent_files_limit);
    let log_filter = data.log_filter.trim();
    let log_filter = if log_filter.is_empty() {
        "info".to_string()
    } else {
        log_filter.to_string()
    };
    AppSettingsData {
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
        log_filter,
        console_drawer_collapsed_default: data.console_drawer_collapsed_default,
        recent_files_limit,
        profiles_directory: data.profiles_directory,
        protontricks_binary_path: data.protontricks_binary_path,
        auto_install_prefix_deps: data.auto_install_prefix_deps,
        discovery_enabled: data.discovery_enabled,
        // Preserve current sources only when the field is omitted.
        // An explicit empty list means "save no sources".
        external_trainer_sources: data
            .external_trainer_sources
            .unwrap_or(current.external_trainer_sources),
        protonup_auto_suggest: data
            .protonup_auto_suggest
            .unwrap_or(current.protonup_auto_suggest),
        protonup_binary_path: data
            .protonup_binary_path
            .unwrap_or(current.protonup_binary_path),
        umu_preference: data.umu_preference.unwrap_or(current.umu_preference),
        // Absent field preserves current value; explicit null clears the timestamp.
        install_nag_dismissed_at: data
            .install_nag_dismissed_at
            .unwrap_or(current.install_nag_dismissed_at),
    }
}

#[tauri::command]
pub fn settings_load(
    store: State<'_, SettingsStore>,
    profile_store: State<'_, crosshook_core::profile::ProfileStore>,
) -> Result<AppSettingsIpcData, String> {
    let data = store.load().map_err(map_settings_error)?;
    let resolved = resolve_profiles_directory_from_config(&data, &store.base_path)
        .map_err(|e| e.to_string())?;
    Ok(AppSettingsIpcData::from_parts(
        data,
        &resolved,
        &profile_store.base_path,
    ))
}

#[tauri::command]
pub fn settings_save(
    data: SettingsSaveRequest,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let current = store.load().map_err(map_settings_error)?;
    let merged = merge_settings_from_request(data, current);
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
pub fn recent_files_load(
    settings_store: State<'_, SettingsStore>,
    store: State<'_, RecentFilesStore>,
) -> Result<RecentFilesData, String> {
    let settings = settings_store.load().map_err(map_settings_error)?;
    let cap = clamp_recent_files_limit(settings.recent_files_limit) as usize;
    store.load(cap).map_err(map_recent_files_error)
}

#[tauri::command]
pub fn recent_files_save(
    data: RecentFilesData,
    settings_store: State<'_, SettingsStore>,
    store: State<'_, RecentFilesStore>,
) -> Result<(), String> {
    let settings = settings_store.load().map_err(map_settings_error)?;
    let cap = clamp_recent_files_limit(settings.recent_files_limit) as usize;
    store.save(&data, cap).map_err(map_recent_files_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = settings_load
            as fn(
                State<'_, SettingsStore>,
                State<'_, crosshook_core::profile::ProfileStore>,
            ) -> Result<AppSettingsIpcData, String>;
        let _ = settings_save
            as fn(SettingsSaveRequest, State<'_, SettingsStore>) -> Result<(), String>;
        let _ = settings_save_steamgriddb_key
            as fn(Option<String>, State<'_, SettingsStore>) -> Result<(), String>;
        let _ = recent_files_load
            as fn(
                State<'_, SettingsStore>,
                State<'_, RecentFilesStore>,
            ) -> Result<RecentFilesData, String>;
        let _ = recent_files_save
            as fn(
                RecentFilesData,
                State<'_, SettingsStore>,
                State<'_, RecentFilesStore>,
            ) -> Result<(), String>;
    }

    #[test]
    fn paths_need_restart_detects_mismatch() {
        let a = PathBuf::from("/tmp/a");
        let b = PathBuf::from("/tmp/b");
        assert!(paths_need_restart_for_profiles(&a, &b));
    }

    fn make_save_request() -> SettingsSaveRequest {
        SettingsSaveRequest {
            auto_load_last_profile: false,
            last_used_profile: String::new(),
            community_taps: vec![],
            onboarding_completed: false,
            offline_mode: false,
            default_proton_path: String::new(),
            default_launch_method: String::new(),
            default_bundled_optimization_preset_id: String::new(),
            default_trainer_loading_mode: String::new(),
            log_filter: "info".to_string(),
            console_drawer_collapsed_default: false,
            recent_files_limit: 10,
            profiles_directory: String::new(),
            protontricks_binary_path: String::new(),
            auto_install_prefix_deps: false,
            discovery_enabled: false,
            external_trainer_sources: None,
            protonup_auto_suggest: None,
            protonup_binary_path: None,
            umu_preference: None,
            install_nag_dismissed_at: None,
        }
    }

    #[test]
    fn merge_preserves_install_nag_dismissed_at_when_omitted() {
        let current = AppSettingsData {
            install_nag_dismissed_at: Some("2026-04-15T12:00:00Z".to_string()),
            ..Default::default()
        };
        let request = make_save_request();
        let merged = merge_settings_from_request(request, current);
        assert_eq!(
            merged.install_nag_dismissed_at,
            Some("2026-04-15T12:00:00Z".to_string()),
            "absent field must preserve existing timestamp"
        );
    }

    #[test]
    fn merge_sets_install_nag_dismissed_at_when_provided() {
        let current = AppSettingsData {
            install_nag_dismissed_at: None,
            ..Default::default()
        };
        let mut request = make_save_request();
        request.install_nag_dismissed_at = Some(Some("2026-04-15T13:00:00Z".to_string()));
        let merged = merge_settings_from_request(request, current);
        assert_eq!(
            merged.install_nag_dismissed_at,
            Some("2026-04-15T13:00:00Z".to_string()),
            "explicit timestamp must be stored"
        );
    }

    #[test]
    fn merge_clears_install_nag_dismissed_at_when_explicitly_null() {
        let current = AppSettingsData {
            install_nag_dismissed_at: Some("2026-04-15T12:00:00Z".to_string()),
            ..Default::default()
        };
        let mut request = make_save_request();
        request.install_nag_dismissed_at = Some(None);
        let merged = merge_settings_from_request(request, current);
        assert!(
            merged.install_nag_dismissed_at.is_none(),
            "explicit null must clear the stored timestamp"
        );
    }

    #[test]
    fn from_parts_surfaces_install_nag_dismissed_at() {
        let data = AppSettingsData {
            install_nag_dismissed_at: Some("2026-04-15T10:00:00Z".to_string()),
            ..Default::default()
        };
        let resolved = PathBuf::from("/tmp/profiles");
        let active = PathBuf::from("/tmp/profiles");
        let ipc = AppSettingsIpcData::from_parts(data, &resolved, &active);
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
        let resolved = PathBuf::from("/tmp/profiles");
        let active = PathBuf::from("/tmp/profiles");
        let ipc = AppSettingsIpcData::from_parts(data, &resolved, &active);
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
            ..Default::default()
        };
        let resolved = PathBuf::from("/tmp/profiles");
        let active = PathBuf::from("/tmp/profiles");
        let ipc = AppSettingsIpcData::from_parts(data, &resolved, &active);
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("install_nag_dismissed_at"),
            "serialized IPC DTO must contain the install_nag_dismissed_at key"
        );
        assert!(
            json.contains(&timestamp),
            "serialized IPC DTO must contain the timestamp value"
        );
    }

    #[test]
    fn settings_ipc_install_nag_null_serializes_as_json_null() {
        let data = AppSettingsData {
            install_nag_dismissed_at: None,
            ..Default::default()
        };
        let resolved = PathBuf::from("/tmp/profiles");
        let active = PathBuf::from("/tmp/profiles");
        let ipc = AppSettingsIpcData::from_parts(data, &resolved, &active);
        let json = serde_json::to_string(&ipc).expect("serialization must not fail");
        assert!(
            json.contains("install_nag_dismissed_at"),
            "serialized IPC DTO must always contain the install_nag_dismissed_at key (as null when unset)"
        );
    }
}
