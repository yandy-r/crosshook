use crosshook_core::metadata::{MetadataStore, MetadataStoreError};
use crosshook_core::profile::{ProfileStore, ProfileStoreError};
use crosshook_core::settings::{AppSettingsData, SettingsStore, SettingsStoreError};
use std::fmt;

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
            })
            .unwrap();

        let resolved = resolve_auto_load_profile_name(&settings_store, &profile_store).unwrap();

        assert_eq!(resolved, None);
    }
}
