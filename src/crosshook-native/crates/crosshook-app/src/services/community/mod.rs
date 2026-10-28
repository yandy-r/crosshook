//! Community tap subscriptions, authorized imports and supervised synchronization.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crosshook_core::community::{CommunityTapStore, CommunityTapSyncResult, TapGitRunner};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use tokio_util::sync::CancellationToken;

use crate::jobs::{JobId, JobRegistry};
use crate::services::ProfileWriter;

mod containment;
mod import;
mod persist;
mod runner;
mod sync;
mod taps;

/// Registry kind for non-exclusive, serialized community synchronization jobs.
pub const COMMUNITY_SYNC_JOB_KIND: &str = "community-sync";
pub(crate) type RunnerFactory =
    Arc<dyn Fn(CancellationToken) -> Arc<dyn TapGitRunner> + Send + Sync>;

/// Toolkit-independent community operations sharing the application's profile writer and jobs.
#[derive(Clone)]
pub struct CommunityService {
    settings: SettingsStore,
    taps: CommunityTapStore,
    profiles: ProfileStore,
    metadata: MetadataStore,
    writer: ProfileWriter,
    jobs: JobRegistry,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
    sync_results: Arc<Mutex<HashMap<JobId, Vec<CommunityTapSyncResult>>>>,
    runner_factory: RunnerFactory,
}

impl CommunityService {
    /// Creates a service with the lifecycle-aware production git runner.
    pub fn new(
        settings: SettingsStore,
        taps: CommunityTapStore,
        profiles: ProfileStore,
        metadata: MetadataStore,
        writer: ProfileWriter,
        jobs: JobRegistry,
    ) -> Self {
        Self {
            settings,
            taps,
            profiles,
            metadata,
            writer,
            jobs,
            sync_lock: Arc::new(tokio::sync::Mutex::new(())),
            sync_results: Arc::new(Mutex::new(HashMap::new())),
            runner_factory: runner::default_runner_factory(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_runner_factory(
        settings: SettingsStore,
        taps: CommunityTapStore,
        profiles: ProfileStore,
        metadata: MetadataStore,
        writer: ProfileWriter,
        jobs: JobRegistry,
        runner_factory: RunnerFactory,
    ) -> Self {
        Self {
            runner_factory,
            ..Self::new(settings, taps, profiles, metadata, writer, jobs)
        }
    }

    #[cfg(test)]
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }
}

#[cfg(test)]
mod tests;
