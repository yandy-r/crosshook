use crosshook_core::metadata::{
    compute_correlation_status, hash_trainer_file, MetadataStore, MetadataStoreError,
    VersionCorrelationStatus,
};
use crosshook_core::profile::{ProfileStore, ProfileStoreError};
use crosshook_core::settings::{AppSettingsData, SettingsStore, SettingsStoreError};
use crosshook_core::steam::discovery::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug)]
pub enum StartupError {
    Metadata(MetadataStoreError),
    Settings(SettingsStoreError),
    Profiles(ProfileStoreError),
}

impl fmt::Display for StartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata(error) => write!(f, "{error}"),
            Self::Settings(error) => write!(f, "{error}"),
            Self::Profiles(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for StartupError {}

impl From<SettingsStoreError> for StartupError {
    fn from(value: SettingsStoreError) -> Self {
        Self::Settings(value)
    }
}

impl From<MetadataStoreError> for StartupError {
    fn from(value: MetadataStoreError) -> Self {
        Self::Metadata(value)
    }
}

impl From<ProfileStoreError> for StartupError {
    fn from(value: ProfileStoreError) -> Self {
        Self::Profiles(value)
    }
}

pub fn run_metadata_reconciliation(
    metadata_store: &MetadataStore,
    profile_store: &ProfileStore,
) -> Result<(), StartupError> {
    let report = metadata_store.sync_profiles_from_store(profile_store)?;
    if report.created > 0 || report.updated > 0 {
        tracing::info!(
            created = report.created,
            updated = report.updated,
            "startup metadata reconciliation complete"
        );
    }
    match metadata_store.sweep_abandoned_operations() {
        Ok(count) if count > 0 => {
            tracing::info!(swept = count, "startup abandoned operation sweep complete");
        }
        Err(error) => {
            tracing::warn!(%error, "startup abandoned operation sweep failed");
        }
        _ => {}
    }
    Ok(())
}

pub fn resolve_auto_load_profile_name(
    settings_store: &SettingsStore,
    profile_store: &ProfileStore,
) -> Result<Option<String>, StartupError> {
    let settings = settings_store.load()?;
    resolve_auto_load_profile_name_from_settings(&settings, profile_store)
}

pub fn resolve_auto_load_profile_name_from_settings(
    settings: &AppSettingsData,
    profile_store: &ProfileStore,
) -> Result<Option<String>, StartupError> {
    if !settings.auto_load_last_profile {
        return Ok(None);
    }

    let last_used_profile = settings.last_used_profile.trim();
    if last_used_profile.is_empty() {
        return Ok(None);
    }

    let available_profiles = profile_store.list()?;
    if available_profiles
        .iter()
        .any(|profile_name| profile_name == last_used_profile)
    {
        return Ok(Some(last_used_profile.to_string()));
    }

    Ok(None)
}

pub async fn run_version_scan(app_handle: AppHandle) {
    let profile_store = app_handle.state::<ProfileStore>();
    let metadata_store = app_handle.state::<MetadataStore>();

    let profile_names = match profile_store.list() {
        Ok(names) => names,
        Err(e) => {
            tracing::warn!(%e, "version scan: failed to list profiles");
            emit_version_scan_complete(&app_handle, 0, 0);
            return;
        }
    };

    let profile_id_map: HashMap<String, String> = metadata_store
        .query_profile_ids_for_names(&profile_names)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut diagnostics = Vec::new();
    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);
    let libraries = discover_steam_libraries(&steam_roots, &mut diagnostics);

    let mut scanned: u32 = 0;
    let mut mismatches: u32 = 0;

    for name in &profile_names {
        let profile = match profile_store.load(name) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(%e, profile_name = %name, "version scan: failed to load profile");
                continue;
            }
        };

        let app_id = profile.steam.app_id.trim().to_string();
        if app_id.is_empty() {
            continue;
        }

        let manifest_path = libraries.iter().find_map(|lib| {
            let path = lib.steamapps_path.join(format!("appmanifest_{app_id}.acf"));
            path.exists().then_some(path)
        });

        let manifest_path = match manifest_path {
            Some(p) => p,
            None => continue,
        };

        let manifest = match parse_manifest_full(&manifest_path) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(profile_name = %name, error = %e, "version scan: manifest parse failed");
                continue;
            }
        };

        // Skip only when StateFlags is present and not 4 (Steam update in progress).
        // None is treated like compute_correlation_status: proceed with comparison.
        if let Some(flags) = manifest.state_flags {
            if flags != 4 {
                continue;
            }
        }

        let profile_id = match profile_id_map.get(name) {
            Some(id) => id.as_str(),
            None => {
                tracing::warn!(profile_name = %name, "version scan: profile_id not in metadata, skipping");
                continue;
            }
        };

        let snapshot = metadata_store
            .lookup_latest_version_snapshot(profile_id)
            .unwrap_or_default();

        let trainer_path = profile.trainer.path.trim().to_string();
        let trainer_hash = if trainer_path.is_empty() {
            None
        } else {
            hash_trainer_file(std::path::Path::new(&trainer_path))
        };

        let snapshot_build_id = snapshot.as_ref().and_then(|s| s.steam_build_id.as_deref());
        let snapshot_trainer_hash = snapshot
            .as_ref()
            .and_then(|s| s.trainer_file_hash.as_deref());

        let status = compute_correlation_status(
            &manifest.build_id,
            snapshot_build_id,
            trainer_hash.as_deref(),
            snapshot_trainer_hash,
            manifest.state_flags,
        );

        scanned += 1;

        if matches!(
            status,
            VersionCorrelationStatus::GameUpdated
                | VersionCorrelationStatus::TrainerChanged
                | VersionCorrelationStatus::BothChanged
        ) {
            mismatches += 1;
        }
    }

    emit_version_scan_complete(&app_handle, scanned, mismatches);
}

fn emit_version_scan_complete(app_handle: &AppHandle, scanned: u32, mismatches: u32) {
    #[derive(Serialize)]
    struct VersionScanComplete {
        scanned: u32,
        mismatches: u32,
    }

    match app_handle.emit(
        "version-scan-complete",
        &VersionScanComplete {
            scanned,
            mismatches,
        },
    ) {
        Ok(()) => tracing::info!(scanned, mismatches, "startup version scan complete"),
        Err(e) => tracing::warn!(%e, "failed to emit version-scan-complete event"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_core::profile::GameProfile;
    use tempfile::tempdir;

    fn store_pair() -> (SettingsStore, ProfileStore) {
        let temp_dir = tempdir().unwrap();
        let settings_store =
            SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
        let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        (settings_store, profile_store)
    }

    #[test]
    fn returns_none_when_auto_load_is_disabled() {
        let (settings_store, profile_store) = store_pair();
        settings_store
            .save(&AppSettingsData {
                auto_load_last_profile: false,
                last_used_profile: "elden-ring".to_string(),
                community_taps: Vec::new(),
                onboarding_completed: false,
                offline_mode: false,
            })
            .unwrap();
        profile_store
            .save("elden-ring", &GameProfile::default())
            .unwrap();

        let resolved = resolve_auto_load_profile_name(&settings_store, &profile_store).unwrap();

        assert_eq!(resolved, None);
    }

    #[test]
    fn returns_some_when_last_used_profile_exists() {
        let (settings_store, profile_store) = store_pair();
        settings_store
            .save(&AppSettingsData {
                auto_load_last_profile: true,
                last_used_profile: "elden-ring".to_string(),
                community_taps: Vec::new(),
                onboarding_completed: false,
                offline_mode: false,
            })
            .unwrap();
        profile_store
            .save("elden-ring", &GameProfile::default())
            .unwrap();

        let resolved = resolve_auto_load_profile_name(&settings_store, &profile_store).unwrap();

        assert_eq!(resolved.as_deref(), Some("elden-ring"));
    }

    #[test]
    fn returns_none_when_last_used_profile_is_missing() {
        let (settings_store, profile_store) = store_pair();
        settings_store
            .save(&AppSettingsData {
                auto_load_last_profile: true,
                last_used_profile: "missing-profile".to_string(),
                community_taps: Vec::new(),
                onboarding_completed: false,
                offline_mode: false,
            })
            .unwrap();

        let resolved = resolve_auto_load_profile_name(&settings_store, &profile_store).unwrap();

        assert_eq!(resolved, None);
    }

    #[test]
    fn returns_none_when_last_used_profile_is_blank() {
        let (settings_store, profile_store) = store_pair();
        settings_store
            .save(&AppSettingsData {
                auto_load_last_profile: true,
                last_used_profile: "   ".to_string(),
                community_taps: Vec::new(),
                onboarding_completed: false,
                offline_mode: false,
            })
            .unwrap();

        let resolved = resolve_auto_load_profile_name(&settings_store, &profile_store).unwrap();

        assert_eq!(resolved, None);
    }
}
