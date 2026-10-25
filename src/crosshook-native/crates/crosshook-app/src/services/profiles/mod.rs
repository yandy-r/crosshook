//! Profile mutation, revision and rollback service.
//!
//! Every mutating facade funnels through one private commit pipeline
//! (`writer.rs`): validate → load → mutate → persist once → observe →
//! revision → health invalidation → publish `AppEvent::ProfilesChanged`.
//! Writes are serialized by a writer-wide lock; reads do not take it.
//! See `docs/plans/profile-writer/b0-contracts.md` for module ownership.

mod defaults_art;
mod history;
mod history_types;
mod imports;
mod launcher_cleanup;
mod lifecycle;
mod migration;
mod presets;
mod reads;
mod revisions;
mod sections;
mod suggestions;
mod types;
mod writer;

use std::sync::{Arc, Mutex};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;

use super::run_blocking;
use crate::EventBus;

pub use history_types::{
    ConfigDiffMode, ConfigDiffResult, ConfigRevisionSummary, ConfigRollbackResult,
    ConfigSemanticChange,
};
pub use types::{
    BundledOptimizationPresetDto, CommandArgumentsPayload, CommitWarning,
    LaunchOptimizationsPayload, ProfileCommit, ProfileSummary,
};

/// Async application service owning every profile write path.
///
/// Clones share the stores, the event bus and the write lock.
#[derive(Clone)]
pub struct ProfileWriter {
    profiles: ProfileStore,
    metadata: MetadataStore,
    settings: SettingsStore,
    events: EventBus,
    /// Serializes mutating commits (D3). Guards `()`, so a poisoned lock is
    /// recovered rather than propagated.
    write_lock: Arc<Mutex<()>>,
}

impl ProfileWriter {
    /// Builds the service from shared persistent stores and the event bus.
    pub fn new(
        profiles: ProfileStore,
        metadata: MetadataStore,
        settings: SettingsStore,
        events: EventBus,
    ) -> Self {
        Self {
            profiles,
            metadata,
            settings,
            events,
            write_lock: Arc::new(Mutex::new(())),
        }
    }
}

#[cfg(test)]
mod tests;
