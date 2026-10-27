//! Shared harness: tempdir profile store, metadata enabled or disabled, and
//! profile / mod fixtures.

use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::mods::{ModCategory, ModProvenance, ProfileModInput};
use crosshook_core::profile::{GameProfile, ProfileStore};
use tempfile::TempDir;

use crate::services::mods::ModsService;

/// Metadata store flavor for a [`Harness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Metadata {
    /// In-memory SQLite store.
    Enabled,
    /// `MetadataStore::disabled()`.
    Disabled,
}

/// A service over tempdir stores.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) root: TempDir,
    pub(super) profiles: ProfileStore,
    pub(super) metadata: MetadataStore,
    pub(super) service: ModsService,
}

impl Harness {
    pub(super) fn new(metadata: Metadata) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let metadata = match metadata {
            Metadata::Enabled => MetadataStore::open_in_memory().unwrap(),
            Metadata::Disabled => MetadataStore::disabled(),
        };
        let service = ModsService::new(profiles.clone(), metadata.clone());
        Self {
            root,
            profiles,
            metadata,
            service,
        }
    }

    /// Saves a profile named `name` whose game executable is `exe` and, when
    /// metadata is available, registers its identity. Returns the profile id.
    pub(super) fn seed_profile(&self, name: &str, exe: &str) -> Option<String> {
        self.save_profile(name, profile_with_exe(exe), true)
    }

    /// Saves a profile without registering a metadata identity.
    pub(super) fn seed_profile_unregistered(&self, name: &str, exe: &str) {
        self.save_profile(name, profile_with_exe(exe), false);
    }

    /// Saves `profile` as `name`; optionally registers its metadata identity.
    pub(super) fn save_profile(
        &self,
        name: &str,
        profile: GameProfile,
        register: bool,
    ) -> Option<String> {
        self.profiles.save(name, &profile).unwrap();
        if !register || !self.metadata.is_available() {
            return None;
        }
        self.metadata
            .observe_profile_write(
                name,
                &profile,
                &self.profiles.base_path.join(format!("{name}.toml")),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
        self.metadata.lookup_profile_id(name).unwrap()
    }

    /// Creates `<root>/game/<relative>` as an empty file.
    pub(super) fn touch_game_file(&self, relative: &str) {
        let path = self.game_dir().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    /// Directory holding the fixture game executable.
    pub(super) fn game_dir(&self) -> std::path::PathBuf {
        self.root.path().join("game")
    }

    /// Absolute path of the fixture game executable.
    pub(super) fn game_exe(&self) -> String {
        self.game_dir()
            .join("game.exe")
            .to_string_lossy()
            .into_owned()
    }
}

/// A profile whose game executable is `exe`.
pub(super) fn profile_with_exe(exe: &str) -> GameProfile {
    let mut profile = GameProfile::default();
    profile.game.name = "Test Game".to_string();
    profile.game.executable_path = exe.to_string();
    profile
}

/// A manual overlay-mod input named `name` with no paths.
pub(super) fn mod_input(name: &str) -> ProfileModInput {
    ProfileModInput {
        name: name.to_string(),
        category: ModCategory::Other,
        paths: Vec::new(),
        enabled: true,
        source_url: None,
        provenance: ModProvenance::Manual,
    }
}
