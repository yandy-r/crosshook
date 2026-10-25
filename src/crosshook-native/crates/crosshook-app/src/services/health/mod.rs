//! Profile health, offline readiness and cached health snapshot services.

mod batch;
mod enrich;
mod prefetch;
mod single;
mod snapshots;
mod steam;
mod types;

use super::run_blocking;
use crate::{AppError, AppErrorKind};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;

pub use types::{
    CachedHealthSnapshot, CachedOfflineReadinessSnapshot, EnrichedHealthSummary,
    EnrichedProfileHealthReport, OfflineReadinessBrief, ProfileHealthMetadata,
};

const FAILURE_TREND_WINDOW_DAYS: u32 = 30;

/// Async application service over profile validation and metadata enrichment.
#[derive(Clone)]
pub struct HealthService {
    profiles: ProfileStore,
    metadata: MetadataStore,
}

impl HealthService {
    /// Builds the service from shared persistent stores.
    pub fn new(profiles: ProfileStore, metadata: MetadataStore) -> Self {
        Self { profiles, metadata }
    }

    /// Validates and enriches every profile on the blocking I/O pool.
    pub async fn batch_summary(&self) -> Result<EnrichedHealthSummary, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || Ok(batch::build_enriched_health_summary(&profiles, &metadata))).await
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
