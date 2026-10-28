//! Harness: tempdir stores, one shared writer, a subscribed event bus and
//! community manifest fixtures.

use std::fs;
use std::path::{Path, PathBuf};

use crosshook_core::community::{
    CommunityProfileManifest, CommunityProfileMetadata, CommunityTapStore,
    CommunityTapSubscription, CompatibilityRating,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

use super::super::{CommunityService, RunnerFactory};
use crate::jobs::JobRegistry;
use crate::services::ProfileWriter;
use crate::{AppEvent, EventBus, EventReceiver};

/// Community service over tempdir stores plus a receiver subscribed before any write.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) root: TempDir,
    pub(super) settings: SettingsStore,
    pub(super) taps: CommunityTapStore,
    pub(super) profiles: ProfileStore,
    pub(super) metadata: MetadataStore,
    pub(super) service: CommunityService,
    pub(super) events: EventReceiver,
}

impl Harness {
    /// Harness with the production runner factory.
    pub(super) fn new() -> Self {
        Self::build(None, None)
    }

    /// Harness whose sync runner comes from `runner_factory`.
    pub(super) fn with_runner_factory(runner_factory: RunnerFactory) -> Self {
        Self::build(Some(runner_factory), None)
    }

    /// Harness over `metadata` (e.g. a disabled store) with the production runner.
    pub(super) fn with_metadata(metadata: MetadataStore) -> Self {
        Self::build(None, Some(metadata))
    }

    fn build(runner_factory: Option<RunnerFactory>, metadata: Option<MetadataStore>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let taps = CommunityTapStore::with_base_path(root.path().join("taps"));
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let metadata = metadata.unwrap_or_else(|| MetadataStore::open_in_memory().unwrap());
        let bus = EventBus::default();
        let events = bus.subscribe();
        let writer = ProfileWriter::new(profiles.clone(), metadata.clone(), settings.clone(), bus);
        let jobs = JobRegistry::default();
        let service = match runner_factory {
            Some(factory) => CommunityService::with_runner_factory(
                settings.clone(),
                taps.clone(),
                profiles.clone(),
                metadata.clone(),
                writer,
                jobs,
                factory,
            ),
            None => CommunityService::new(
                settings.clone(),
                taps.clone(),
                profiles.clone(),
                metadata.clone(),
                writer,
                jobs,
            ),
        };
        Self {
            root,
            settings,
            taps,
            profiles,
            metadata,
            service,
            events,
        }
    }

    /// Next already-published event, or `None` when nothing is queued.
    pub(super) async fn next_event(&mut self) -> Option<AppEvent> {
        tokio::time::timeout(std::time::Duration::from_millis(50), self.events.recv())
            .await
            .ok()
            .flatten()
    }

    /// Replaces the persisted subscriptions verbatim (extras included).
    pub(super) fn set_taps(&self, taps: Vec<CommunityTapSubscription>) {
        self.settings
            .update(|settings| {
                settings.community_taps = taps;
                Ok::<_, ()>(())
            })
            .unwrap()
            .unwrap();
    }

    /// Persisted subscriptions exactly as stored (extras included).
    pub(super) fn saved_taps(&self) -> Vec<CommunityTapSubscription> {
        self.settings.load().unwrap().community_taps
    }

    /// Raw bytes of the settings file; empty when it was never written.
    pub(super) fn settings_bytes(&self) -> Vec<u8> {
        fs::read(self.settings.settings_path()).unwrap_or_default()
    }

    /// Local clone location the store derives for `tap`.
    pub(super) fn workspace_path(&self, tap: &CommunityTapSubscription) -> PathBuf {
        self.taps.resolve_workspace(tap).unwrap().local_path
    }

    /// Persists `url` as the only subscription and creates its workspace directory.
    pub(super) fn subscribe_workspace(&self, url: &str) -> PathBuf {
        let subscription = tap(url);
        self.set_taps(vec![subscription.clone()]);
        let workspace = self.workspace_path(&subscription);
        fs::create_dir_all(&workspace).unwrap();
        workspace
    }
}

/// A subscription for `url` on the default branch, unpinned, without extras.
pub(super) fn tap(url: &str) -> CommunityTapSubscription {
    CommunityTapSubscription {
        url: url.to_string(),
        ..CommunityTapSubscription::default()
    }
}

/// A valid community manifest for `game_name`.
pub(super) fn manifest(game_name: &str) -> CommunityProfileManifest {
    CommunityProfileManifest::new(
        CommunityProfileMetadata {
            game_name: game_name.to_string(),
            game_version: "1.0".to_string(),
            trainer_name: "Trainer".to_string(),
            trainer_version: "1".to_string(),
            proton_version: "9".to_string(),
            platform_tags: vec!["linux".to_string()],
            compatibility_rating: CompatibilityRating::Working,
            author: "Author".to_string(),
            description: "Test profile".to_string(),
            trainer_sha256: None,
        },
        GameProfile::default(),
    )
}

/// Writes an importable manifest JSON (explicit `schema_version`) at `path`,
/// creating parent directories.
pub(super) fn write_manifest(path: &Path, game_name: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut value = serde_json::to_value(manifest(game_name)).unwrap();
    value.as_object_mut().unwrap().insert(
        "schema_version".to_string(),
        serde_json::Value::from(crosshook_core::community::COMMUNITY_PROFILE_SCHEMA_VERSION),
    );
    fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}
