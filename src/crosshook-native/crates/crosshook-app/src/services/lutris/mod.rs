//! Lutris import service: read-only library preview and the shared-writer
//! import of the selected entries.

mod import;

use crosshook_core::profile::ProfileStore;

use super::ProfileWriter;

/// Async application service over the Lutris library importer.
///
/// Clones share the profile store and the single [`ProfileWriter`], so an
/// import serializes with every other profile write and publishes the shared
/// `Imported` event.
#[derive(Clone)]
pub struct LutrisService {
    profiles: ProfileStore,
    writer: ProfileWriter,
}

impl LutrisService {
    /// Builds the service from the shared profile store and profile writer.
    pub fn new(profiles: ProfileStore, writer: ProfileWriter) -> Self {
        Self { profiles, writer }
    }
}

static_assertions::assert_impl_all!(LutrisService: Clone, Send, Sync);

#[cfg(test)]
mod tests;
