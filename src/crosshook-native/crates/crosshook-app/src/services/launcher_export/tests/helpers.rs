//! Shared harness: tempdir stores, a home directory, request and profile
//! builders, and launcher-file readers. Every helper is used by a slice.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crosshook_core::export::SteamExternalLauncherExportRequest;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, GamescopeConfig, ProfileStore, TrainerLoadingMode};
use crosshook_core::settings::{SettingsStore, UmuPreference};
use tempfile::TempDir;

use super::super::LauncherExportService;

/// Metadata store flavor for a [`Harness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Metadata {
    /// In-memory SQLite store.
    Enabled,
    /// `MetadataStore::disabled()`.
    Disabled,
}

pub(super) struct Harness {
    pub(super) _root: TempDir,
    pub(super) home: PathBuf,
    pub(super) profiles: ProfileStore,
    pub(super) settings: SettingsStore,
    pub(super) metadata: MetadataStore,
    pub(super) service: LauncherExportService,
}

impl Harness {
    pub(super) fn new(metadata: Metadata) -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = match metadata {
            Metadata::Enabled => MetadataStore::open_in_memory().unwrap(),
            Metadata::Disabled => MetadataStore::disabled(),
        };
        let service =
            LauncherExportService::new(profiles.clone(), settings.clone(), metadata.clone());
        Self {
            _root: root,
            home,
            profiles,
            settings,
            metadata,
            service,
        }
    }

    pub(super) fn home_str(&self) -> String {
        self.home.to_string_lossy().into_owned()
    }

    /// Writes a stub trainer and returns an export request for `launcher_name`.
    pub(super) fn request(&self, launcher_name: &str) -> SteamExternalLauncherExportRequest {
        let trainer = self
            .home
            .join("trainers")
            .join(format!("{launcher_name}.exe"));
        fs::create_dir_all(trainer.parent().unwrap()).unwrap();
        fs::write(&trainer, b"MZ stub").unwrap();
        SteamExternalLauncherExportRequest {
            method: "proton_run".to_string(),
            launcher_name: launcher_name.to_string(),
            trainer_path: trainer.to_string_lossy().into_owned(),
            trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
            launcher_icon_path: String::new(),
            prefix_path: "/fake/prefix".to_string(),
            proton_path: "/fake/proton/proton".to_string(),
            steam_app_id: "12345".to_string(),
            steam_client_install_path: "/fake/steam".to_string(),
            target_home_path: self.home_str(),
            profile_name: None,
            runtime_steam_app_id: String::new(),
            umu_game_id: String::new(),
            umu_preference: UmuPreference::Auto,
            network_isolation: false,
            gamescope: GamescopeConfig::default(),
            custom_env_vars: Default::default(),
        }
    }

    pub(super) fn launchers_dir(&self) -> PathBuf {
        self.home.join(".local/share/crosshook/launchers")
    }

    pub(super) fn applications_dir(&self) -> PathBuf {
        self.home.join(".local/share/applications")
    }

    pub(super) fn script_path(&self, slug: &str) -> PathBuf {
        self.launchers_dir().join(format!("{slug}-trainer.sh"))
    }

    pub(super) fn desktop_path(&self, slug: &str) -> PathBuf {
        self.applications_dir()
            .join(format!("crosshook-{slug}-trainer.desktop"))
    }

    /// Saves a `proton_run` profile whose launcher name is `display_name`.
    pub(super) fn save_profile(&self, name: &str, display_name: &str) -> GameProfile {
        let request = self.request(display_name);
        let mut profile = GameProfile::default();
        profile.launch.method = "proton_run".to_string();
        profile.game.name = name.to_string();
        profile.trainer.path = request.trainer_path;
        profile.steam.app_id = request.steam_app_id;
        profile.steam.launcher.display_name = display_name.to_string();
        profile.runtime.prefix_path = request.prefix_path;
        profile.runtime.proton_path = request.proton_path;
        self.profiles.save(name, &profile).unwrap();
        profile
    }

    /// Makes the settings file unreadable as TOML.
    pub(super) fn corrupt_settings(&self) {
        fs::create_dir_all(&self.settings.base_path).unwrap();
        fs::write(self.settings.settings_path(), "not = [valid toml").unwrap();
    }

    /// Counts `launchers` rows with `slug` and `drift_state`.
    pub(super) fn launcher_rows(&self, slug: &str, drift_state: &str) -> i64 {
        self.metadata
            .with_sqlite_conn("count launcher rows", |conn| {
                Ok(conn
                    .query_row(
                        "SELECT COUNT(*) FROM launchers WHERE launcher_slug = ?1 AND drift_state = ?2",
                        rusqlite::params![slug, drift_state],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap())
            })
            .unwrap()
    }
}

pub(super) fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}
