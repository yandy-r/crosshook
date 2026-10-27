//! Version service: Steam build and trainer version correlation reads and writes.

#![expect(
    dead_code,
    reason = "B2 fills the methods; B4/B5 removes this once everything is wired"
)]

mod ops;
mod status;

use crosshook_core::metadata::{MetadataStore, VersionCorrelationStatus};
use crosshook_core::profile::ProfileStore;
use serde::Serialize;

/// Async application service over version snapshots and correlation.
#[derive(Clone)]
pub struct VersionService {
    profiles: ProfileStore,
    metadata: MetadataStore,
}

impl VersionService {
    /// Builds the service from shared stores.
    pub fn new(profiles: ProfileStore, metadata: MetadataStore) -> Self {
        Self { profiles, metadata }
    }
}

/// A recorded version snapshot, mirroring `VersionSnapshotRow` with `status`
/// parsed to the typed [`VersionCorrelationStatus`] enum.
#[derive(Debug, Clone, Serialize)]
pub struct VersionSnapshotInfo {
    /// Profile id.
    pub profile_id: String,
    /// Steam app id.
    pub steam_app_id: String,
    /// Steam build id at snapshot time.
    pub steam_build_id: Option<String>,
    /// Trainer version at snapshot time.
    pub trainer_version: Option<String>,
    /// Trainer file hash at snapshot time.
    pub trainer_file_hash: Option<String>,
    /// Human-entered game version.
    pub human_game_ver: Option<String>,
    /// Correlation status recorded with the snapshot.
    pub status: VersionCorrelationStatus,
    /// Snapshot timestamp.
    pub checked_at: String,
}

/// Result of a version check: the live manifest state combined with the latest
/// recorded snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct VersionCheckResult {
    /// Profile id (empty when the profile is not registered).
    pub profile_id: String,
    /// Build id read from the live Steam manifest.
    pub current_build_id: Option<String>,
    /// Latest recorded snapshot.
    pub snapshot: Option<VersionSnapshotInfo>,
    /// Computed correlation status.
    pub status: VersionCorrelationStatus,
    /// Whether Steam is mid-update for the game.
    pub update_in_progress: bool,
}

static_assertions::assert_impl_all!(VersionService: Send, Sync);

#[cfg(test)]
mod tests;
