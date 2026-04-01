//! Application settings persistence helpers.

use directories::BaseDirs;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::community::CommunityTapSubscription;

pub mod recent;

pub use recent::{RecentFilesData, RecentFilesStore, RecentFilesStoreError};

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub base_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub steamgriddb_api_key: Option<String>,
}

#[derive(Debug)]
pub enum SettingsStoreError {
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for SettingsStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDe(error) => write!(f, "{error}"),
            Self::TomlSer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SettingsStoreError {}

impl From<std::io::Error> for SettingsStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for SettingsStoreError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for SettingsStoreError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore {
    pub fn try_new() -> Result<Self, String> {
        let base_path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .config_dir()
            .join("crosshook");
        Ok(Self { base_path })
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook settings storage")
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn load(&self) -> Result<AppSettingsData, SettingsStoreError> {
        fs::create_dir_all(&self.base_path)?;

        let path = self.settings_path();
        if !path.exists() {
            return Ok(AppSettingsData::default());
        }

        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError> {
        fs::create_dir_all(&self.base_path)?;
        fs::write(self.settings_path(), toml::to_string_pretty(settings)?)?;
        Ok(())
    }

    pub fn settings_path(&self) -> PathBuf {
        self.base_path.join("settings.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_settings_when_file_is_missing() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

        let settings = store.load().unwrap();

        assert_eq!(settings, AppSettingsData::default());
        assert!(store.settings_path().parent().unwrap().exists());
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
        let settings = AppSettingsData {
            auto_load_last_profile: true,
            last_used_profile: "elden-ring".to_string(),
            community_taps: vec![CommunityTapSubscription {
                url: "https://example.invalid/community.git".to_string(),
                branch: Some("main".to_string()),
                pinned_commit: Some("deadbeef".to_string()),
            }],
            onboarding_completed: true,
            offline_mode: false,
            steamgriddb_api_key: None,
        };

        store.save(&settings).unwrap();

        assert_eq!(store.load().unwrap(), settings);
        assert!(store.settings_path().exists());
    }

    #[test]
    fn onboarding_completed_defaults_to_false_when_absent() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

        fs::create_dir_all(&store.base_path).unwrap();
        // TOML that deliberately omits onboarding_completed
        fs::write(
            store.settings_path(),
            "auto_load_last_profile = true\nlast_used_profile = \"elden-ring\"\n",
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert!(!settings.onboarding_completed);
    }

    #[test]
    fn offline_mode_defaults_false_when_absent() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

        fs::create_dir_all(&store.base_path).unwrap();
        fs::write(
            store.settings_path(),
            "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert!(!settings.offline_mode);
    }

    #[test]
    fn load_uses_missing_fields_defaults() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

        fs::create_dir_all(&store.base_path).unwrap();
        fs::write(
            store.settings_path(),
            "last_used_profile = \"elden-ring\"\n",
        )
        .unwrap();

        let settings = store.load().unwrap();

        assert_eq!(
            settings,
            AppSettingsData {
                auto_load_last_profile: false,
                last_used_profile: "elden-ring".to_string(),
                community_taps: Vec::new(),
                onboarding_completed: false,
                offline_mode: false,
                steamgriddb_api_key: None,
            },
        );
    }
}
