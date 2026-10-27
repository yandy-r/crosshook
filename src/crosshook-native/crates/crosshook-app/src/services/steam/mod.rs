//! Steam service: client install path, Proton discovery, auto-populate and game
//! metadata / cover art.

mod client;
mod metadata;
mod proton;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;

/// Async application service over Steam discovery, Proton installs and game
/// metadata / art.
#[derive(Clone)]
pub struct SteamService {
    metadata: MetadataStore,
    settings: SettingsStore,
}

impl SteamService {
    /// Builds the service from shared stores.
    pub fn new(metadata: MetadataStore, settings: SettingsStore) -> Self {
        Self { metadata, settings }
    }
}

static_assertions::assert_impl_all!(SteamService: Send, Sync);

#[cfg(test)]
mod tests;
