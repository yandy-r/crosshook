//! Proton migration discovery and profile updates through the shared writer.

use crosshook_core::profile::ProfileStore;

use crate::services::{ProfileWriter, SteamService};

mod apply;
mod check;

/// Toolkit-independent Proton migration operations sharing the profile writer.
#[derive(Clone)]
pub struct ProtonMigrationService {
    profiles: ProfileStore,
    writer: ProfileWriter,
    steam: SteamService,
}

impl ProtonMigrationService {
    /// Builds the service from the shared profile store, writer and Steam service.
    pub fn new(profiles: ProfileStore, writer: ProfileWriter, steam: SteamService) -> Self {
        Self {
            profiles,
            writer,
            steam,
        }
    }
}

static_assertions::assert_impl_all!(ProtonMigrationService: Send, Sync);

#[cfg(test)]
mod tests;
