//! `ProtonMigrationService` tests. This file is the shared harness; `check`
//! and `apply` own their slices.

mod apply;
mod check;

use std::fs;
use std::path::{Path, PathBuf};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

use super::ProtonMigrationService;
use crate::services::{ProfileWriter, SteamService};
use crate::{AppEvent, EventBus, EventReceiver};

/// Service over tempdir stores plus a receiver subscribed before any write.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) root: TempDir,
    pub(super) profiles: ProfileStore,
    pub(super) service: ProtonMigrationService,
    pub(super) events: EventReceiver,
}

impl Harness {
    pub(super) fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = MetadataStore::open_in_memory().unwrap();
        let bus = EventBus::default();
        let events = bus.subscribe();
        let writer = ProfileWriter::new(profiles.clone(), metadata.clone(), settings.clone(), bus);
        let steam = SteamService::new(metadata, settings);
        let service = ProtonMigrationService::new(profiles.clone(), writer, steam);
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

    /// Saves a `proton_run` profile whose `runtime.proton_path` is `proton_path`.
    pub(super) fn seed_proton_run(&self, name: &str, proton_path: &str) {
        let mut profile = GameProfile::default();
        profile.game.name = name.to_string();
        profile.launch.method = "proton_run".to_string();
        profile.runtime.proton_path = proton_path.to_string();
        self.profiles.save(name, &profile).unwrap();
    }
}

/// Creates `<root>/<dir>/proton` and returns its path.
pub(super) fn proton_file(root: &Path, dir: &str) -> PathBuf {
    let proton_dir = root.join(dir);
    fs::create_dir_all(&proton_dir).unwrap();
    let path = proton_dir.join("proton");
    fs::write(&path, b"#!/bin/sh\n").unwrap();
    path
}
