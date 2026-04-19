//! Application settings persistence helpers.

use directories::BaseDirs;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use crate::community::CommunityTapSubscription;
use crate::discovery::models::{
    default_external_trainer_sources, ExternalTrainerSourceSubscription,
};

pub mod recent;

pub use recent::{RecentFilesData, RecentFilesStore, RecentFilesStoreError};

/// Minimum allowed `recent_files_limit` (inclusive).
pub const RECENT_FILES_LIMIT_MIN: u32 = 1;
/// Maximum allowed `recent_files_limit` (inclusive).
pub const RECENT_FILES_LIMIT_MAX: u32 = 100;

fn default_log_filter() -> String {
    "info".to_string()
}

fn default_protonup_auto_suggest() -> bool {
    true
}

fn default_protonup_default_provider() -> String {
    "ge-proton".to_string()
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

fn current_user_home() -> Result<PathBuf, String> {
    BaseDirs::new()
        .ok_or_else(|| {
            "home directory not found — CrossHook requires a user home directory".to_string()
        })
        .map(|dirs| dirs.home_dir().to_path_buf())
}

/// Resolve `~username` to that user's home directory.
#[cfg(unix)]
fn resolve_user_home(username: &str) -> Result<PathBuf, String> {
    use std::process::Command;

    let output = if crate::platform::is_flatpak() {
        crate::platform::host_std_command("getent")
    } else {
        Command::new("getent")
    }
    .args(["passwd", username])
    .output()
    .map_err(|e| format!("failed to look up user '{username}': {e}"))?;

    if !output.status.success() {
        return Err(format!("user '{username}' not found"));
    }

    let line = String::from_utf8_lossy(&output.stdout);
    let home_field = line
        .split(':')
        .nth(5)
        .ok_or_else(|| format!("could not determine home directory for user '{username}'"))?;

    let home = PathBuf::from(home_field.trim());
    if home.as_os_str().is_empty() {
        return Err(format!("home directory for user '{username}' is empty"));
    }
    Ok(home)
}

#[cfg(not(unix))]
fn resolve_user_home(username: &str) -> Result<PathBuf, String> {
    Err(format!(
        "~{username} expansion is not supported on this platform"
    ))
}

pub(crate) fn expand_path_with_tilde(raw: &str) -> Result<PathBuf, String> {
    let t = raw.trim();

    // ~/path — current user's home
    if let Some(rest) = t.strip_prefix("~/") {
        return Ok(current_user_home()?.join(rest));
    }

    // bare ~ — current user's home (canonicalized)
    if t == "~" {
        return current_user_home()?
            .canonicalize()
            .map_err(|e| e.to_string());
    }

    // ~username or ~username/path — named user's home
    if let Some(after_tilde) = t.strip_prefix('~') {
        let (username, subpath) = match after_tilde.find('/') {
            Some(i) => (&after_tilde[..i], Some(&after_tilde[i + 1..])),
            None => (after_tilde, None),
        };
        let home = resolve_user_home(username)?;
        return Ok(match subpath {
            Some(p) => home.join(p),
            None => home,
        });
    }

    Ok(PathBuf::from(t))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UmuPreference {
    #[default]
    Auto,
    Umu,
    Proton,
}

impl UmuPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Umu => "umu",
            Self::Proton => "proton",
        }
    }
}

impl FromStr for UmuPreference {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "auto" => Ok(Self::Auto),
            "umu" => Ok(Self::Umu),
            "proton" => Ok(Self::Proton),
            other => Err(format!("unsupported umu preference: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub base_path: PathBuf,
    io_lock: Arc<Mutex<()>>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    /// High-contrast UI toggle for accessibility. Defaults to false.
    #[serde(default)]
    pub high_contrast: bool,
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
    /// Path override for winetricks/protontricks binary; empty = auto-detect from PATH.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protontricks_binary_path: String,
    /// When true, auto-install missing prefix deps on first launch.
    #[serde(default)]
    pub auto_install_prefix_deps: bool,
    /// User opt-in for trainer discovery feature (external links to third-party sources).
    #[serde(default)]
    pub discovery_enabled: bool,
    /// User-managed list of external trainer discovery source subscriptions.
    /// FLiNG ships as the built-in default via `default_external_trainer_sources()`.
    #[serde(default = "default_external_trainer_sources")]
    pub external_trainer_sources: Vec<ExternalTrainerSourceSubscription>,
    /// When true, show ProtonUp runtime suggestions for community profiles.
    #[serde(default = "default_protonup_auto_suggest")]
    pub protonup_auto_suggest: bool,
    /// Optional path override for ProtonUp binary; empty = auto-detect.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protonup_binary_path: String,
    /// Which provider to preselect in the Proton manager UI (e.g. `"ge-proton"`).
    #[serde(default = "default_protonup_default_provider")]
    pub protonup_default_provider: String,
    /// Override the auto-picked install root for Proton downloads; empty = auto-resolve.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protonup_default_install_root: String,
    /// When true, include GitHub prereleases in the Proton catalog.
    #[serde(default)]
    pub protonup_include_prereleases: bool,
    /// User preference for umu-launcher vs direct Proton invocation.
    #[serde(default)]
    pub umu_preference: UmuPreference,
    /// Capability-level host-tool hints the user chose to dismiss permanently.
    #[serde(default)]
    pub host_tool_dashboard_dismissed_hints: Vec<String>,
    /// Optional default category filter for the host-tool dashboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_tool_dashboard_default_category_filter: Option<String>,
    /// RFC 3339 timestamp of when the user dismissed the umu install nag; `None` = not dismissed.
    ///
    /// TODO(#269): Canonical dismissals live in SQLite `readiness_nag_dismissals`; this field remains
    /// for settings.toml backward compatibility and startup migration into the DB.
    pub install_nag_dismissed_at: Option<String>,
    /// RFC 3339 timestamp of when the user dismissed the Steam Deck gaming-mode caveats;
    /// `None` = not dismissed.
    ///
    /// TODO(#269): Canonical dismissals live in SQLite `readiness_nag_dismissals`; this field remains
    /// for settings.toml backward compatibility and startup migration into the DB.
    pub steam_deck_caveats_dismissed_at: Option<String>,
}

impl Default for AppSettingsData {
    fn default() -> Self {
        Self {
            auto_load_last_profile: false,
            last_used_profile: String::new(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            high_contrast: false,
            steamgriddb_api_key: None,
            default_proton_path: String::new(),
            default_launch_method: String::new(),
            default_bundled_optimization_preset_id: String::new(),
            default_trainer_loading_mode: "source_directory".to_string(),
            log_filter: default_log_filter(),
            console_drawer_collapsed_default: true,
            recent_files_limit: default_recent_files_limit(),
            profiles_directory: String::new(),
            protontricks_binary_path: String::new(),
            auto_install_prefix_deps: false,
            discovery_enabled: false,
            external_trainer_sources: default_external_trainer_sources(),
            protonup_auto_suggest: default_protonup_auto_suggest(),
            protonup_binary_path: String::new(),
            protonup_default_provider: default_protonup_default_provider(),
            protonup_default_install_root: String::new(),
            protonup_include_prereleases: false,
            umu_preference: UmuPreference::Auto,
            host_tool_dashboard_dismissed_hints: Vec::new(),
            host_tool_dashboard_default_category_filter: None,
            install_nag_dismissed_at: None,
            steam_deck_caveats_dismissed_at: None,
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
            .field("high_contrast", &self.high_contrast)
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
            .field("protontricks_binary_path", &self.protontricks_binary_path)
            .field("auto_install_prefix_deps", &self.auto_install_prefix_deps)
            .field("discovery_enabled", &self.discovery_enabled)
            .field("external_trainer_sources", &self.external_trainer_sources)
            .field("protonup_auto_suggest", &self.protonup_auto_suggest)
            .field("protonup_binary_path", &self.protonup_binary_path)
            .field("protonup_default_provider", &self.protonup_default_provider)
            .field(
                "protonup_default_install_root",
                &self.protonup_default_install_root,
            )
            .field(
                "protonup_include_prereleases",
                &self.protonup_include_prereleases,
            )
            .field("umu_preference", &self.umu_preference)
            .field(
                "host_tool_dashboard_dismissed_hints",
                &self.host_tool_dashboard_dismissed_hints,
            )
            .field(
                "host_tool_dashboard_default_category_filter",
                &self.host_tool_dashboard_default_category_filter,
            )
            .field("install_nag_dismissed_at", &self.install_nag_dismissed_at)
            .field(
                "steam_deck_caveats_dismissed_at",
                &self.steam_deck_caveats_dismissed_at,
            )
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
        Ok(Self {
            base_path,
            io_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook settings storage")
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self {
            base_path,
            io_lock: Arc::new(Mutex::new(())),
        }
    }

    fn load_unlocked(&self) -> Result<AppSettingsData, SettingsStoreError> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(AppSettingsData::default());
        }

        let content = fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(Into::into)
    }

    fn save_unlocked(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError> {
        fs::write(self.settings_path(), toml::to_string_pretty(settings)?)?;
        Ok(())
    }

    pub fn load(&self) -> Result<AppSettingsData, SettingsStoreError> {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;
        self.load_unlocked()
    }

    pub fn save(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError> {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;
        self.save_unlocked(settings)
    }

    /// Explicitly writes normalized settings to disk for callers that opt-in
    /// to backfilling newly added fields. Returns true if the file changed.
    pub fn migrate_or_save_settings(
        &self,
        settings: &AppSettingsData,
    ) -> Result<bool, SettingsStoreError> {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;

        let path = self.settings_path();
        let serialized = toml::to_string_pretty(settings)?;
        let should_write = match fs::read_to_string(&path) {
            Ok(content) => content != serialized,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(SettingsStoreError::Io(error)),
        };

        if should_write {
            fs::write(path, serialized)?;
        }

        Ok(should_write)
    }

    /// Atomically load-mutate-save settings under a single process-local lock.
    /// The file is only written if `mutator` returns `Ok(_)`.
    pub fn update<F, T, E>(&self, mutator: F) -> Result<Result<T, E>, SettingsStoreError>
    where
        F: FnOnce(&mut AppSettingsData) -> Result<T, E>,
    {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;

        let mut settings = self.load_unlocked()?;
        let result = mutator(&mut settings);
        if result.is_ok() {
            self.save_unlocked(&settings)?;
        }
        Ok(result)
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
    fn high_contrast_defaults_false_when_absent() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

        fs::create_dir_all(&store.base_path).unwrap();
        fs::write(
            store.settings_path(),
            "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
        )
        .unwrap();

        let settings = store.load().unwrap();
        assert!(
            !settings.high_contrast,
            "high_contrast should default to false when not present in settings.toml"
        );
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

    #[test]
    fn settings_roundtrip_with_protontricks_fields() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
        let settings = AppSettingsData {
            protontricks_binary_path: "/usr/bin/protontricks".to_string(),
            auto_install_prefix_deps: true,
            ..Default::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.protontricks_binary_path, "/usr/bin/protontricks");
        assert!(loaded.auto_install_prefix_deps);
    }

    #[test]
    fn settings_backward_compat_without_protontricks_fields() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
        // Save settings without the new fields (simulate old config)
        let old_toml = "auto_load_last_profile = false\nlast_used_profile = \"\"\n";
        std::fs::create_dir_all(store.settings_path().parent().unwrap()).unwrap();
        std::fs::write(store.settings_path(), old_toml).unwrap();
        let loaded = store.load().unwrap();
        assert!(loaded.protontricks_binary_path.is_empty());
        assert!(!loaded.auto_install_prefix_deps);
    }

    #[test]
    fn settings_backward_compat_without_protonup_fields() {
        let temp_dir = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
        // Old TOML that has no protonup_* keys — new fields must fall back to defaults.
        let old_toml = "auto_load_last_profile = false\nlast_used_profile = \"\"\n";
        std::fs::create_dir_all(store.settings_path().parent().unwrap()).unwrap();
        std::fs::write(store.settings_path(), old_toml).unwrap();
        let loaded = store.load().unwrap();
        assert!(
            loaded.protonup_auto_suggest,
            "protonup_auto_suggest should default to true"
        );
        assert!(
            loaded.protonup_binary_path.is_empty(),
            "protonup_binary_path should default to empty"
        );
    }

    #[test]
    fn settings_backward_compat_without_protonup_manager_fields() {
        // TOML that has pre-v22 settings including protonup_auto_suggest / protonup_binary_path
        // but NOT the three new manager fields. Must parse cleanly and fill defaults.
        let legacy_toml = r#"
protonup_auto_suggest = true
protonup_binary_path = ""
# deliberately omit protonup_default_provider, protonup_default_install_root, protonup_include_prereleases
        "#;
        let parsed: AppSettingsData = toml::from_str(legacy_toml).expect("parses legacy toml");
        assert_eq!(parsed.protonup_default_provider, "ge-proton");
        assert_eq!(parsed.protonup_default_install_root, "");
        assert!(!parsed.protonup_include_prereleases);
    }

    #[test]
    fn settings_roundtrip_protonup_manager_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let settings = AppSettingsData {
            protonup_default_provider: "proton-cachyos".to_string(),
            protonup_default_install_root: "/home/user/.steam/root/compatibilitytools.d"
                .to_string(),
            protonup_include_prereleases: true,
            ..Default::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.protonup_default_provider, "proton-cachyos");
        assert_eq!(
            loaded.protonup_default_install_root,
            "/home/user/.steam/root/compatibilitytools.d"
        );
        assert!(loaded.protonup_include_prereleases);
    }

    #[test]
    fn settings_backward_compat_without_umu_preference() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
        )
        .unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.umu_preference, UmuPreference::Auto);
    }

    #[test]
    fn settings_backward_compat_without_install_nag_dismissed_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
        )
        .unwrap();
        let loaded = store.load().unwrap();
        assert!(
            loaded.install_nag_dismissed_at.is_none(),
            "install_nag_dismissed_at should default to None when absent from settings.toml"
        );
    }

    #[test]
    fn settings_save_roundtrip_preserves_install_nag_dismissed_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let timestamp = "2026-04-15T12:00:00Z".to_string();
        let settings = AppSettingsData {
            install_nag_dismissed_at: Some(timestamp.clone()),
            ..Default::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.install_nag_dismissed_at,
            Some(timestamp),
            "install_nag_dismissed_at must survive a save/load roundtrip"
        );
    }

    #[test]
    fn settings_backward_compat_without_steam_deck_caveats_dismissed_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
        )
        .unwrap();
        let loaded = store.load().unwrap();
        assert!(
            loaded.steam_deck_caveats_dismissed_at.is_none(),
            "steam_deck_caveats_dismissed_at should default to None when absent from settings.toml"
        );
    }

    #[test]
    fn settings_roundtrip_steam_deck_caveats_dismissed_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let timestamp = "2026-04-15T12:00:00Z".to_string();
        let settings = AppSettingsData {
            steam_deck_caveats_dismissed_at: Some(timestamp.clone()),
            ..Default::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.steam_deck_caveats_dismissed_at,
            Some(timestamp),
            "steam_deck_caveats_dismissed_at must survive a save/load roundtrip"
        );
    }

    #[test]
    fn settings_roundtrip_umu_preference_umu() {
        let toml = "umu_preference = \"umu\"\n";
        let parsed: AppSettingsData = toml::from_str(toml).unwrap();
        assert_eq!(parsed.umu_preference, UmuPreference::Umu);
        let serialized = toml::to_string(&parsed).unwrap();
        assert!(serialized.contains("umu_preference = \"umu\""));
    }

    #[test]
    fn settings_backward_compat_without_host_tool_dashboard_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
        )
        .unwrap();
        let loaded = store.load().unwrap();
        assert!(loaded.host_tool_dashboard_dismissed_hints.is_empty());
        assert!(loaded.host_tool_dashboard_default_category_filter.is_none());
    }

    #[test]
    fn settings_roundtrip_host_tool_dashboard_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(dir.path().to_path_buf());
        let settings = AppSettingsData {
            host_tool_dashboard_dismissed_hints: vec![
                "gamescope".to_string(),
                "prefix_tools".to_string(),
            ],
            host_tool_dashboard_default_category_filter: Some("runtime".to_string()),
            ..Default::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.host_tool_dashboard_dismissed_hints,
            vec!["gamescope".to_string(), "prefix_tools".to_string()]
        );
        assert_eq!(
            loaded.host_tool_dashboard_default_category_filter,
            Some("runtime".to_string())
        );
    }

    #[test]
    fn umu_preference_from_str_rejects_unknown() {
        use std::str::FromStr;
        assert!(UmuPreference::from_str("ghoti").is_err());
        assert_eq!(UmuPreference::from_str("umu").unwrap(), UmuPreference::Umu);
        assert_eq!(
            UmuPreference::from_str("auto").unwrap(),
            UmuPreference::Auto
        );
        assert_eq!(
            UmuPreference::from_str("proton").unwrap(),
            UmuPreference::Proton
        );
    }
}
