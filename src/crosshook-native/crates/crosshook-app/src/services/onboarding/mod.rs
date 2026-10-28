//! Onboarding readiness checks, cached host snapshot, capabilities, nag
//! dismissals (TOML + SQLite dual write) and trainer guidance.

mod dismissals;
mod guidance;
mod readiness;
mod snapshot;

#[cfg(test)]
mod tests;

pub use dismissals::LegacyNag;
pub use snapshot::CachedHostReadinessSnapshot;

use std::sync::Arc;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;

use super::host_tools::{CoreHostProbe, HostProbe};
use crate::jobs::JobRegistry;

/// Async onboarding and readiness service. Clones are cheap.
#[derive(Clone)]
pub struct OnboardingService {
    settings: SettingsStore,
    metadata: MetadataStore,
    jobs: JobRegistry,
    probe: Arc<dyn HostProbe>,
}

impl OnboardingService {
    /// Builds the service over the live host probe.
    pub fn new(settings: SettingsStore, metadata: MetadataStore, jobs: JobRegistry) -> Self {
        Self::with_probe(settings, metadata, jobs, Arc::new(CoreHostProbe))
    }

    /// Builds the service over an injected host probe (tests, fixtures).
    pub fn with_probe(
        settings: SettingsStore,
        metadata: MetadataStore,
        jobs: JobRegistry,
        probe: Arc<dyn HostProbe>,
    ) -> Self {
        Self {
            settings,
            metadata,
            jobs,
            probe,
        }
    }
}
