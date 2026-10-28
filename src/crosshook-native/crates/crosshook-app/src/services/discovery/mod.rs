//! Toolkit-independent discovery catalog, external search and source management.

use crosshook_core::community::CommunityTapStore;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;

mod catalog;
mod compatibility;
mod search;
mod sources;
#[cfg(test)]
mod tests;

/// Discovery operations over the shared settings, metadata and tap stores.
#[derive(Clone)]
pub struct DiscoveryService {
    settings: SettingsStore,
    metadata: MetadataStore,
    taps: CommunityTapStore,
}

impl DiscoveryService {
    /// Creates a service using the application's existing stores.
    pub fn new(settings: SettingsStore, metadata: MetadataStore, taps: CommunityTapStore) -> Self {
        Self {
            settings,
            metadata,
            taps,
        }
    }
}

static_assertions::assert_impl_all!(DiscoveryService: Send, Sync);
