use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::protondb::{
    cache_key_for_app_id, ProtonDbLookupResult, ProtonDbLookupState, ProtonDbSnapshot, ProtonDbTier,
};
use tempfile::TempDir;

use super::super::ProtonDbService;

pub(super) struct Harness {
    pub(super) _root: TempDir,
    pub(super) metadata: MetadataStore,
    pub(super) profiles: ProfileStore,
    pub(super) service: ProtonDbService,
}

impl Harness {
    pub(super) fn new(metadata: MetadataStore) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let service = ProtonDbService::new(metadata.clone(), profiles.clone());
        Self {
            _root: root,
            metadata,
            profiles,
            service,
        }
    }

    pub(super) fn profile(&self, observe: bool) {
        let profile = GameProfile::default();
        self.profiles
            .save_without_companion("Game", &profile)
            .unwrap();
        if observe {
            self.metadata
                .observe_profile_write(
                    "Game",
                    &profile,
                    &self.profiles.base_path.join("Game.toml"),
                    SyncSource::AppWrite,
                    None,
                )
                .unwrap();
        }
    }

    pub(super) fn seed_cache(&self, app_id: &str, stale: bool) {
        let payload = ProtonDbLookupResult {
            app_id: app_id.into(),
            state: ProtonDbLookupState::Ready,
            cache: None,
            snapshot: Some(ProtonDbSnapshot {
                app_id: app_id.into(),
                tier: ProtonDbTier::Gold,
                total_reports: Some(12),
                source_url: "https://www.protondb.com/app/test".into(),
                ..Default::default()
            }),
        };
        let expires =
            (chrono::Utc::now() + chrono::Duration::hours(if stale { -1 } else { 6 })).to_rfc3339();
        self.metadata
            .put_cache_entry(
                "https://www.protondb.com/app/test",
                &cache_key_for_app_id(app_id),
                &serde_json::to_string(&payload).unwrap(),
                Some(&expires),
            )
            .unwrap();
    }
}
