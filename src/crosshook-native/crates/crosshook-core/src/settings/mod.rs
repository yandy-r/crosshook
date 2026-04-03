//! Application settings persistence helpers.

use directories::BaseDirs;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::community::CommunityTapSubscription;

pub mod recent;

pub use recent::{RecentFilesData, RecentFilesStore, RecentFilesStoreError};

/// Minimum allowed `recent_files_limit` (inclusive).
pub const RECENT_FILES_LIMIT_MIN: u32 = 1;
/// Maximum allowed `recent_files_limit` (inclusive).
pub const RECENT_FILES_LIMIT_MAX: u32 = 100;

fn default_log_filter() -> String {
    "info".to_string()
}

fn default_recent_files_limit() -> u32 {
    10
}

fn default_console_drawer_collapsed() -> bool {
    true
}

/// Clamp recent-files list size to a safe range for persistence and UI.
pub fn clamp_recent_files_limit(n: u32) -> u32 {
    n.clamp(RECENT_FILES_LIMIT_MIN, RECENT_FILES_LIMIT_MAX)
}

/// Resolve the profiles directory: empty config uses `crosshook_config_dir/profiles`.
pub fn resolve_profiles_directory_from_config(
    settings: &AppSettingsData,
    crosshook_config_dir: &Path,
) -> Result<PathBuf, String> {
    let raw = settings.profiles_directory.trim();
    if raw.is_empty() {
        return Ok(crosshook_config_dir.join("profiles"));
    }
    expand_path_with_tilde(raw)
}

fn expand_path_with_tilde(raw: &str) -> Result<PathBuf, String> {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("~/") {
        let home = BaseDirs::new()
            .ok_or_else(|| "home directory not found — CrossHook requires a user home directory".to_string())?
            .home_dir()
            .to_path_buf();
        return Ok(home.join(rest));
    }
    if t == "~" {
        return BaseDirs::new()
            .ok_or_else(|| "home directory not found — CrossHook requires a user home directory".to_string())?
            .home_dir()
            .canonicalize()
            .map_err(|e| e.to_string());
    }
    Ok(PathBuf::from(t))
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub base_path: PathBuf,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub steamgriddb_api_key: Option<String>,
    /// Default Proton path applied to new profiles when `runtime.proton_path` is empty.
    pub default_proton_path: String,
    /// Default launch method (`proton_run`, `steam_applaunch`, `native`, …) for new profiles.
    pub default_launch_method: String,
    /// Bundled optimization preset id (metadata `preset_id`) for new profiles; empty = none.
    pub default_bundled_optimization_preset_id: String,
    /// `source_directory` or `copy_to_prefix`.
    pub default_trainer_loading_mode: String,
    /// Tracing filter when `RUST_LOG` is unset (e.g. `info`, `debug`, `crosshook_core=debug`).
    #[serde(default = "default_log_filter")]
    pub log_filter: String,
    /// Initial console drawer collapsed state before any log-driven auto-expand.
    #[serde(default = "default_console_drawer_collapsed")]
    pub console_drawer_collapsed_default: bool,
    /// Max recent paths per category; clamped on save.
    #[serde(default = "default_recent_files_limit")]
    pub recent_files_limit: u32,
    /// Override profiles directory; empty = default under config. Restart required to apply.
    pub profiles_directory: String,
}

impl Default for AppSettingsData {
    fn default() -> Self {
        Self {
            auto_load_last_profile: false,
            last_used_profile: String::new(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            steamgriddb_api_key: None,
            default_proton_path: String::new(),
            default_launch_method: String::new(),
            default_bundled_optimization_preset_id: String::new(),
            default_trainer_loading_mode: "source_directory".to_string(),
            log_filter: default_log_filter(),
            console_drawer_collapsed_default: true,
            recent_files_limit: default_recent_files_limit(),
            profiles_directory: String::new(),
        }
    }
}

impl fmt::Debug for AppSettingsData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppSettingsData")
            .field("auto_load_last_profile", &self.auto_load_last_profile)
            .field("last_used_profile", &self.last_used_profile)
            .field("community_taps", &self.community_taps)
            .field("onboarding_completed", &self.onboarding_completed)
            .field("offline_mode", &self.offline_mode)
            .field(
                "steamgriddb_api_key",
                &self
                    .steamgriddb_api_key
                    .as_ref()
                    .map(|_| "<redacted>")
                    .unwrap_or("<none>"),
            )
            .field("default_proton_path", &self.default_proton_path)
            .field("default_launch_method", &self.default_launch_method)
            .field(
                "default_bundled_optimization_preset_id",
                &self.default_bundled_optimization_preset_id,
            )
            .field(
                "default_trainer_loading_mode",
                &self.default_trainer_loading_mode,
            )
            .field("log_filter", &self.log_filter)
            .field(
                "console_drawer_collapsed_default",
                &self.console_drawer_collapsed_default,
            )
            .field("recent_files_limit", &self.recent_files_limit)
            .field("profiles_directory", &self.profiles_directory)
            .finish()
    }
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
    use std::path::PathBuf;
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
            ..Default::default()
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
                ..Default::default()
            },
        );
    }

    #[test]
    fn resolve_profiles_directory_default_under_config() {
        let temp = tempdir().unwrap();
        let cfg = temp.path().join("crosshook");
        let s = AppSettingsData::default();
        let p = resolve_profiles_directory_from_config(&s, &cfg).unwrap();
        assert_eq!(p, cfg.join("profiles"));
    }

    #[test]
    fn resolve_profiles_directory_custom_tilde() {
        let temp = tempdir().unwrap();
        let cfg = temp.path().join("crosshook");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        // Pretend home via env is not used — expand_path uses BaseDirs which uses real home.
        // Test only default branch; tilde test in integration if needed.
        let s = AppSettingsData {
            profiles_directory: temp.path().join("myprofiles").display().to_string(),
            ..Default::default()
        };
        let p = resolve_profiles_directory_from_config(&s, &cfg).unwrap();
        assert_eq!(p, PathBuf::from(s.profiles_directory));
    }
}
