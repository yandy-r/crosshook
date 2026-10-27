//! Collections service: CRUD, membership, launch defaults (including the profile
//! defaults merge) and TOML export/import preview.

mod crud;
mod defaults;
mod transfer;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;

/// Async application service over collections, their defaults and TOML transfer.
#[derive(Clone)]
pub struct CollectionsService {
    metadata: MetadataStore,
    profiles: ProfileStore,
}

impl CollectionsService {
    /// Builds the service from shared stores.
    pub fn new(metadata: MetadataStore, profiles: ProfileStore) -> Self {
        Self { metadata, profiles }
    }
}

static_assertions::assert_impl_all!(CollectionsService: Send, Sync);

#[cfg(test)]
mod tests;
