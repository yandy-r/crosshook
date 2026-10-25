//! Shared harness: tempdir stores, metadata enabled or disabled, a subscribed
//! event bus and a Steam profile fixture.

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{
    GameProfile, GameSection, LaunchSection, LauncherSection, ProfileStore, SteamSection,
    TrainerLoadingMode, TrainerSection,
};
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

use crate::services::profiles::ProfileWriter;
use crate::{AppEvent, EventBus, EventReceiver};

/// Metadata store flavor for a [`Harness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Metadata {
    /// In-memory SQLite store.
    Enabled,
    /// `MetadataStore::disabled()`.
    Disabled,
}

/// A writer over tempdir stores plus a receiver subscribed before any write.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) root: TempDir,
    pub(super) profiles: ProfileStore,
    pub(super) settings: SettingsStore,
    pub(super) metadata: MetadataStore,
    pub(super) writer: ProfileWriter,
    pub(super) events: EventReceiver,
}

impl Harness {
    pub(super) fn new(metadata: Metadata) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = match metadata {
            Metadata::Enabled => MetadataStore::open_in_memory().unwrap(),
            Metadata::Disabled => MetadataStore::disabled(),
        };
        let bus = EventBus::default();
        let events = bus.subscribe();
        let writer = ProfileWriter::new(profiles.clone(), metadata.clone(), settings.clone(), bus);
        Self {
            root,
            profiles,
            settings,
            metadata,
            writer,
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
}

/// A Steam profile whose compatdata lives under `home`.
pub(super) fn steam_profile(home: &str) -> GameProfile {
    GameProfile {
        game: GameSection {
            extra: toml::Table::new(),
            name: "Test Game".to_string(),
            executable_path: String::new(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection {
            extra: toml::Table::new(),
            path: "/tmp/trainers/test.exe".to_string(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        steam: SteamSection {
            app_id: "12345".to_string(),
            compatdata_path: format!("{home}/.local/share/Steam/steamapps/compatdata/12345"),
            launcher: LauncherSection {
                extra: toml::Table::new(),
                display_name: "Test Game".to_string(),
                icon_path: String::new(),
            },
            ..Default::default()
        },
        launch: LaunchSection {
            method: "steam_applaunch".to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Sources of the stored config revisions of `name`, newest first.
pub(super) fn revision_sources(metadata: &MetadataStore, name: &str) -> Vec<String> {
    let profile_id = metadata.lookup_profile_id(name).unwrap().unwrap();
    metadata
        .list_config_revisions(&profile_id, None)
        .unwrap()
        .into_iter()
        .map(|row| row.source)
        .collect()
}

/// `(source, source_profile_id)` of the live metadata row named `name`.
pub(super) fn sync_row(metadata: &MetadataStore, name: &str) -> (String, Option<String>) {
    metadata
        .with_sqlite_conn("read profile sync row", |conn| {
            Ok(conn
                .query_row(
                    "SELECT source, source_profile_id FROM profiles \
                     WHERE current_filename = ?1 AND deleted_at IS NULL",
                    [name],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap())
        })
        .unwrap()
}
