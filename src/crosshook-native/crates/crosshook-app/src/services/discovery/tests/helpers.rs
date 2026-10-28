use crosshook_core::community::CommunityTapStore;
use crosshook_core::discovery::{fling_default_source, ExternalTrainerSearchQuery};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

use super::super::DiscoveryService;

pub(super) struct Harness {
    pub(super) _root: TempDir,
    pub(super) settings: SettingsStore,
    pub(super) metadata: MetadataStore,
    pub(super) service: DiscoveryService,
}

impl Harness {
    pub(super) fn new(metadata: MetadataStore) -> Self {
        let root = tempfile::tempdir().unwrap();
        let settings = SettingsStore::with_base_path(root.path().join("settings"));
        let taps = CommunityTapStore::with_base_path(root.path().join("taps"));
        let service = DiscoveryService::new(settings.clone(), metadata.clone(), taps);
        Self {
            _root: root,
            settings,
            metadata,
            service,
        }
    }

    pub(super) fn offline(&self) {
        self.settings
            .update(|settings| {
                settings.offline_mode = true;
                settings.external_trainer_sources = vec![fling_default_source()];
                Ok::<_, ()>(())
            })
            .unwrap()
            .unwrap();
    }

    pub(super) fn seed_cache(&self, stale: bool, empty: bool) {
        let expires =
            (chrono::Utc::now() + chrono::Duration::hours(if stale { -2 } else { 1 })).to_rfc3339();
        let payload = if empty {
            "[]"
        } else {
            r#"[{"title":"Elden Ring Trainer","link":"https://example.com/elden","pub_date":null}]"#
        };
        self.metadata
            .put_cache_entry(
                "https://example.com/",
                "trainer:source:v1:fling:elden_ring",
                payload,
                Some(&expires),
            )
            .unwrap();
    }
}

pub(super) fn query() -> ExternalTrainerSearchQuery {
    ExternalTrainerSearchQuery {
        game_name: "Elden Ring".into(),
        steam_app_id: None,
        force_refresh: Some(true),
    }
}
