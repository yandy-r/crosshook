//! Launcher export service: validate, export, preview, list and delete external
//! launcher scripts and desktop entries.

mod export;
mod list;
mod preview;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;

/// Async application service over launcher export, preview, list and delete.
#[derive(Clone)]
pub struct LauncherExportService {
    profiles: ProfileStore,
    settings: SettingsStore,
    metadata: MetadataStore,
}

impl LauncherExportService {
    /// Builds the service from shared stores.
    pub fn new(profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore) -> Self {
        Self {
            profiles,
            settings,
            metadata,
        }
    }
}

static_assertions::assert_impl_all!(LauncherExportService: Send, Sync);

#[cfg(test)]
mod tests;
