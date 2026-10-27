//! Shared harness for `CollectionsService` tests.

use std::path::PathBuf;

use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::profile::{CollectionDefaultsSection, GameProfile, ProfileStore};
use tempfile::TempDir;

use crate::services::collections::CollectionsService;

/// A service over an in-memory metadata store and a temp profile directory.
pub(super) struct Harness {
    pub(super) service: CollectionsService,
    pub(super) metadata: MetadataStore,
    pub(super) profiles: ProfileStore,
    pub(super) dir: TempDir,
}

impl Harness {
    /// Available in-memory metadata store.
    pub(super) fn new() -> Self {
        Self::with_metadata(MetadataStore::open_in_memory().expect("open in-memory store"))
    }

    /// Disabled metadata store (`is_available()` is false).
    pub(super) fn disabled() -> Self {
        Self::with_metadata(MetadataStore::disabled())
    }

    fn with_metadata(metadata: MetadataStore) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let profiles = ProfileStore::with_base_path(dir.path().join("profiles"));
        let service = CollectionsService::new(metadata.clone(), profiles.clone());
        Self {
            service,
            metadata,
            profiles,
            dir,
        }
    }

    /// Saves `profile` to disk and registers it in the metadata store.
    pub(super) fn register_profile(&self, name: &str, profile: &GameProfile) {
        self.profiles.save(name, profile).expect("save profile");
        self.metadata
            .observe_profile_write(
                name,
                profile,
                &PathBuf::from("/profiles").join(format!("{name}.toml")),
                SyncSource::AppWrite,
                None,
            )
            .expect("observe profile write");
    }

    /// Registers a default profile named `name` (game name = `name`).
    pub(super) fn register_named(&self, name: &str) {
        let mut profile = GameProfile::default();
        profile.game.name = name.to_string();
        self.register_profile(name, &profile);
    }

    /// Creates a collection with the given defaults and returns its id.
    pub(super) fn collection_with_defaults(
        &self,
        name: &str,
        defaults: &CollectionDefaultsSection,
    ) -> String {
        let id = self
            .metadata
            .create_collection(name)
            .expect("create collection");
        self.metadata
            .set_collection_defaults(&id, Some(defaults))
            .expect("seed defaults");
        id
    }

    /// Writes raw text into `defaults_json`, bypassing validation.
    pub(super) fn corrupt_defaults(&self, collection_id: &str) {
        self.metadata
            .with_sqlite_conn("seed corrupt defaults", |conn| {
                conn.execute(
                    "UPDATE collections SET defaults_json = ?1 WHERE collection_id = ?2",
                    ("{not-valid-json", collection_id),
                )
                .expect("raw update");
                Ok(())
            })
            .expect("with_sqlite_conn");
    }

    /// Path inside the harness temp dir.
    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }
}

/// A profile with one custom env var.
pub(super) fn profile_with_env(name: &str, value: &str) -> GameProfile {
    let mut profile = GameProfile::default();
    profile
        .launch
        .custom_env_vars
        .insert(name.to_string(), value.to_string());
    profile
}
