//! Harness: tempdir stores, one shared writer, a subscribed event bus and a
//! minimal real Lutris library fixture.

use std::fs;
use std::path::{Path, PathBuf};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

use crate::services::lutris::LutrisService;
use crate::services::profiles::ProfileWriter;
use crate::{AppEvent, EventBus, EventReceiver};

/// Lutris service over tempdir stores plus a receiver subscribed before any write.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) root: TempDir,
    pub(super) profiles: ProfileStore,
    pub(super) service: LutrisService,
    pub(super) events: EventReceiver,
}

impl Harness {
    pub(super) fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let bus = EventBus::default();
        let events = bus.subscribe();
        let writer = ProfileWriter::new(
            profiles.clone(),
            MetadataStore::open_in_memory().unwrap(),
            settings,
            bus,
        );
        let service = LutrisService::new(profiles.clone(), writer);
        Self {
            root,
            profiles,
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

    /// Writes a one-game Lutris library under the harness root and returns it.
    pub(super) fn lutris_library(&self) -> PathBuf {
        let library = self.root.path().join("lutris");
        write_game_config(&library, "sample-game", "Sample Game");
        library
    }
}

/// Writes `games/<stem>.yml`, a Wine game that maps to an importable profile.
pub(super) fn write_game_config(library: &Path, stem: &str, name: &str) {
    let games = library.join("games");
    fs::create_dir_all(&games).unwrap();
    fs::write(
        games.join(format!("{stem}.yml")),
        format!(
            "name: {name}\n\
             game:\n  exe: /games/{stem}/game.exe\n  prefix: ~/Games/{stem}-prefix\n\
             wine:\n  version: lutris-GE-Proton8-14-x86_64\n"
        ),
    )
    .unwrap();
}
