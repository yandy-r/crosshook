//! Toolkit-independent ProtonDB lookup and suggestion operations.

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;

mod lookup;
mod suggestions;
#[cfg(test)]
mod tests;

/// Number of days a dismissed suggestion is retained.
const SUGGESTION_DISMISSAL_RETENTION_DAYS: u32 = 30;

/// ProtonDB operations over the application's existing metadata and profiles.
#[derive(Clone)]
pub struct ProtonDbService {
    metadata: MetadataStore,
    profiles: ProfileStore,
}

impl ProtonDbService {
    /// Creates a service using the application's existing stores.
    pub fn new(metadata: MetadataStore, profiles: ProfileStore) -> Self {
        Self { metadata, profiles }
    }
}

static_assertions::assert_impl_all!(ProtonDbService: Send, Sync);
