//! Shared harness for `VersionService` tests.

use std::fs;
use std::path::PathBuf;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, ProfileStore};
use tempfile::TempDir;

use super::super::VersionService;

/// A service over tempdir stores plus a fixture Steam library tree.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) _root: TempDir,
    pub(super) profiles: ProfileStore,
    pub(super) metadata: MetadataStore,
    pub(super) service: VersionService,
    /// Fixture Steam root (`<root>/steam`); its `steamapps` is a library.
    pub(super) steam_root: PathBuf,
    /// Fixture trainer file (`<root>/trainer.exe`).
    pub(super) trainer_path: PathBuf,
}

impl Harness {
    pub(super) fn new() -> Self {
        Self::with_metadata(MetadataStore::open_in_memory().unwrap())
    }

    pub(super) fn disabled() -> Self {
        Self::with_metadata(MetadataStore::disabled())
    }

    fn with_metadata(metadata: MetadataStore) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let steam_root = root.path().join("steam");
        fs::create_dir_all(steam_root.join("steamapps")).unwrap();
        let trainer_path = root.path().join("trainer.exe");
        fs::write(&trainer_path, b"trainer-bytes").unwrap();
        let service = VersionService::new(profiles.clone(), metadata.clone());
        Self {
            _root: root,
            profiles,
            metadata,
            service,
            steam_root,
            trainer_path,
        }
    }

    /// Saves a profile for `name` pointing at the fixture Steam tree and
    /// trainer file. Does not register it in metadata; call
    /// [`Self::register`] for that.
    pub(super) fn save_profile(&self, name: &str) -> GameProfile {
        let mut profile = GameProfile::default();
        profile.game.name = "Test Game".to_string();
        profile.steam.app_id = "12345".to_string();
        profile.steam.compatdata_path = format!(
            "{}/steamapps/compatdata/12345",
            self.steam_root.to_string_lossy()
        );
        profile.trainer.path = self.trainer_path.to_string_lossy().into_owned();
        self.profiles.save(name, &profile).unwrap();
        profile
    }

    /// Saves and metadata-registers `name`, returning its profile id.
    pub(super) fn registered_profile(&self, name: &str) -> String {
        self.save_profile(name);
        self.metadata
            .sync_profiles_from_store(&self.profiles)
            .unwrap();
        self.metadata.lookup_profile_id(name).unwrap().unwrap()
    }

    /// Writes `appmanifest_12345.acf` with the given build id and StateFlags.
    pub(super) fn write_manifest(&self, build_id: &str, state_flags: u32) {
        let content = format!(
            "\"AppState\"\n{{\n  \"appid\" \"12345\"\n  \"installdir\" \"MyGame\"\n  \"buildid\" \"{build_id}\"\n  \"StateFlags\" \"{state_flags}\"\n}}\n"
        );
        fs::write(
            self.steam_root.join("steamapps/appmanifest_12345.acf"),
            content,
        )
        .unwrap();
    }
}
