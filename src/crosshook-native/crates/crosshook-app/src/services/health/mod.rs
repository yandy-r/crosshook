//! Profile health, offline readiness and cached health snapshot services.

mod batch;
mod enrich;
mod prefetch;
mod single;
mod snapshots;
mod steam;
mod types;

use super::{run_blocking, run_job};
use crate::jobs::{JobRegistry, JobSpec};
use crate::{AppError, AppErrorKind};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;

pub use types::{
    CachedHealthSnapshot, CachedOfflineReadinessSnapshot, EnrichedHealthSummary,
    EnrichedProfileHealthReport, OfflineReadinessBrief, ProfileHealthMetadata,
};

const FAILURE_TREND_WINDOW_DAYS: u32 = 30;
const HEALTH_BATCH_JOB_KIND: &str = "health-batch";

/// Async application service over profile validation and metadata enrichment.
#[derive(Clone)]
pub struct HealthService {
    profiles: ProfileStore,
    metadata: MetadataStore,
    jobs: JobRegistry,
}

impl HealthService {
    /// Builds the service from shared persistent stores and the app job
    /// registry that supervises batch validation.
    pub fn new(profiles: ProfileStore, metadata: MetadataStore, jobs: JobRegistry) -> Self {
        Self {
            profiles,
            metadata,
            jobs,
        }
    }

    /// Validates and enriches every profile as a supervised `health-batch`
    /// job on the blocking I/O pool (cancelled on app shutdown). Not
    /// exclusive: the startup scan and UI refreshes may overlap.
    pub async fn batch_summary(&self) -> Result<EnrichedHealthSummary, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        let spec = JobSpec::new(HEALTH_BATCH_JOB_KIND);
        run_job(&self.jobs, spec, move || {
            Ok(batch::build_enriched_health_summary(&profiles, &metadata))
        })
        .await
    }

    /// Returns enriched health for one named profile.
    pub async fn profile_health(
        &self,
        name: String,
    ) -> Result<EnrichedProfileHealthReport, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            single::get_profile_health(name, &profiles, &metadata)
                .map_err(|error| AppError::new(AppErrorKind::Unavailable, error))
        })
        .await
    }

    /// Reads health snapshots retained by the last batch validation.
    pub async fn cached_health_snapshots(&self) -> Result<Vec<CachedHealthSnapshot>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            snapshots::get_cached_health_snapshots(&metadata)
                .map_err(|error| AppError::new(AppErrorKind::Unavailable, error))
        })
        .await
    }

    /// Reads cached offline readiness snapshots without scanning profiles.
    pub async fn cached_offline_readiness_snapshots(
        &self,
    ) -> Result<Vec<CachedOfflineReadinessSnapshot>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            snapshots::get_cached_offline_readiness_snapshots(&metadata)
                .map_err(|error| AppError::new(AppErrorKind::Unavailable, error))
        })
        .await
    }
}

#[cfg(test)]
mod tests;
