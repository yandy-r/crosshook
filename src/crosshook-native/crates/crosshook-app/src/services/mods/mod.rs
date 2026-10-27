//! Profile mods service: registry reads and writes, directory detection and
//! coexistence analysis.

#![expect(
    dead_code,
    reason = "B2 fills the methods; B4/B5 removes this once everything is wired"
)]

mod coexistence;
mod crud;
mod detect;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::mods::ProfileModRecord;
use crosshook_core::profile::ProfileStore;
use serde::Serialize;

/// Async application service over the per-profile mod registry.
#[derive(Clone)]
pub struct ModsService {
    profiles: ProfileStore,
    metadata: MetadataStore,
}

impl ModsService {
    /// Builds the service from shared stores.
    pub fn new(profiles: ProfileStore, metadata: MetadataStore) -> Self {
        Self { profiles, metadata }
    }
}

/// Registered mods of one profile; `available` is `false` when the metadata
/// database is unavailable.
#[derive(Debug, Serialize)]
pub struct ProfileModsResponse {
    /// Whether the metadata database could be read.
    pub available: bool,
    /// Registered mods.
    pub mods: Vec<ProfileModRecord>,
}

static_assertions::assert_impl_all!(ModsService: Send, Sync);

#[cfg(test)]
mod tests;
