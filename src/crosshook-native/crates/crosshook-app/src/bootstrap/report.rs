//! The startup report: a live, idempotent snapshot of startup state.
//!
//! [`StartupReport`] is what `get_startup_report` returns and what the React
//! shell pulls on mount. It is cheap to clone and safe to re-fetch: a late
//! subscriber never misses a job completion, because results accumulate in the
//! startup state *before* the completion event is published.
//!
//! # Frozen vs live fields
//!
//! Every field is classified, and any future field must declare which kind it
//! is in this table:
//!
//! - **Live** (recomputed on every snapshot, so a webview reload after
//!   dismissing onboarding does not re-show it): [`StartupReport::auto_load_profile`],
//!   [`StartupReport::onboarding`].
//! - **Frozen** (produced once by [`pre_runtime`](super::pre_runtime) or
//!   `App::bootstrap`, then carried verbatim): [`StartupReport::flatpak`],
//!   [`StartupReport::degraded`].
//! - **Accumulating** (written by startup jobs, state-then-publish):
//!   [`StartupReport::jobs`].
//!
//! # JSON contract
//!
//! Serialization matches the hand-written TypeScript contract
//! (`src/types/startup.ts`): snake_case keys, [`JobState`] internally tagged
//! `"state"`, and `degraded.metadata` mirroring `MetadataStatus`
//! (`{"state":"ok"}` and friends). The exact shapes are pinned by unit tests
//! in this module.

use crosshook_core::metadata::MetadataStatus;
use serde::Serialize;
use std::sync::Arc;

use super::pre_runtime::FlatpakMode;
use crate::services::background::BackgroundProtectionState;
use crate::services::health::EnrichedHealthSummary;

/// Onboarding prompt state for the startup shell.
///
/// Live field: recomputed from settings and the profile store on every
/// snapshot, so reloading the webview after the wizard is dismissed does not
/// re-show it.
#[derive(Debug, Clone, Serialize)]
pub struct OnboardingStartup {
    /// Whether the onboarding wizard should be shown now.
    pub show: bool,
    /// Whether at least one profile already exists.
    pub has_profiles: bool,
}

/// Result of the first-run Flatpak migration, sanitized for the UI.
///
/// Frozen field: produced once by Phase A. `error` carries the sanitized
/// failure detail when the best-effort migration failed; migration failures
/// are never fatal.
#[derive(Debug, Clone, Serialize)]
pub struct FlatpakMigrationReport {
    /// Whether the legacy config file was imported.
    pub imported_config: bool,
    /// Data subtrees that were imported.
    pub imported_subtrees: Vec<String>,
    /// Data subtrees that were intentionally not imported.
    pub skipped_subtrees: Vec<String>,
    /// Sanitized migration failure detail, if the migration failed.
    pub error: Option<String>,
}

/// Flatpak gate outcome carried into the report.
///
/// Frozen field: resolved during Phase A before runtime construction.
#[derive(Debug, Clone, Serialize)]
pub struct FlatpakStartup {
    /// How the Flatpak gate resolved this process.
    pub mode: FlatpakMode,
    /// First-run migration outcome, when the gate ran it.
    pub migration: Option<FlatpakMigrationReport>,
}

/// Non-fatal degradation of optional startup facilities.
///
/// Frozen field: determined once during Phase B store initialization.
#[derive(Debug, Clone, Serialize)]
pub struct DegradedStores {
    /// Sanitized metadata store health (same JSON shape as `metadata_store_status`).
    pub metadata: MetadataStatus,
    /// Sanitized reason the metadata store is degraded, when it is.
    pub metadata_error: Option<String>,
    /// Whether `settings.toml` was unreadable and defaults are in use.
    pub settings_unreadable: bool,
    /// Whether the log-file subscriber could not be installed.
    pub logging_unavailable: bool,
}

/// Coarse state of one startup job, independent of its payload.
///
/// Returned by [`StartupHandle::wait_finished`](super::StartupHandle::wait_finished)
/// so tests and the native UI can await a job without depending on its data
/// type. Mirrors the [`JobState`] variants one-to-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobSnapshotKind {
    /// Not started yet.
    Pending,
    /// Started, not finished.
    Running,
    /// Finished successfully; data is in the snapshot.
    Complete,
    /// Finished with an error; message is in the snapshot.
    Failed,
    /// Not run, with an explicit reason.
    Skipped,
}

/// Lifecycle of one startup job, carrying its payload when complete.
///
/// Accumulating field: written by startup jobs (state first, then the
/// completion event is published), so a snapshot taken after completion still
/// contains the result. Serialized internally tagged as `"state"` with
/// snake_case variant names.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState<T> {
    /// Not started yet.
    Pending,
    /// Started, not finished.
    Running,
    /// Finished successfully with its data.
    Complete {
        /// Job result.
        data: T,
    },
    /// Finished with a sanitized error message.
    Failed {
        /// Sanitized failure detail.
        message: String,
    },
    /// Not run, with an explicit reason (e.g. `CROSSHOOK_BENCH=1`).
    Skipped {
        /// Why the job did not run.
        reason: String,
    },
}

impl<T> JobState<T> {
    /// Returns the lifecycle state without inspecting or cloning its payload.
    pub fn kind(&self) -> JobSnapshotKind {
        match self {
            Self::Pending => JobSnapshotKind::Pending,
            Self::Running => JobSnapshotKind::Running,
            Self::Complete { .. } => JobSnapshotKind::Complete,
            Self::Failed { .. } => JobSnapshotKind::Failed,
            Self::Skipped { .. } => JobSnapshotKind::Skipped,
        }
    }
}
/// Small offline-readiness notification payload (legacy event shape).
#[derive(Debug, Clone, Serialize)]
pub struct OfflineReadinessScanSummary {
    /// Number of profiles the readiness scan covered.
    pub total_profiles: usize,
}

/// Version-correlation scan payload (legacy event shape).
#[derive(Debug, Clone, Serialize)]
pub struct VersionScanSummary {
    /// Profiles whose manifest and trainer hash were compared.
    pub scanned: u32,
    /// Profiles whose game or trainer version changed since the snapshot.
    pub mismatches: u32,
}

/// umu compatibility-database refresh payload.
#[derive(Debug, Clone, Serialize)]
pub struct UmuRefreshSummary {
    /// Whether the database was refreshed this run.
    pub refreshed: bool,
    /// Cache timestamp of the refreshed database, when known.
    pub cached_at: Option<String>,
    /// Why the refresh did not run, when it was skipped.
    pub reason: String,
}

/// Startup job results, one slot per [`StartupJobKind`](crate::events::StartupJobKind).
///
/// Accumulating field: each slot is written exactly once by its job.
#[derive(Debug, Clone, Serialize)]
pub struct StartupJobs {
    /// Enriched profile health batch (same shape as `profile-health-batch-complete`).
    pub health: JobState<Arc<EnrichedHealthSummary>>,
    /// Offline readiness scan notification.
    pub offline_readiness: JobState<OfflineReadinessScanSummary>,
    /// Steam/trainer version correlation scan.
    pub version_scan: JobState<VersionScanSummary>,
    /// umu compatibility-database refresh.
    pub umu_refresh: JobState<UmuRefreshSummary>,
    /// Initial Flatpak background portal request.
    pub background_portal: JobState<BackgroundProtectionState>,
}

/// Live, idempotent snapshot of startup state served to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct StartupReport {
    /// Profile to auto-load on mount, when settings and the store agree.
    ///
    /// Live field.
    pub auto_load_profile: Option<String>,
    /// Onboarding prompt state.
    ///
    /// Live field.
    pub onboarding: OnboardingStartup,
    /// Flatpak gate and first-run migration outcome.
    ///
    /// Frozen field.
    pub flatpak: FlatpakStartup,
    /// Degraded optional facilities.
    ///
    /// Frozen field.
    pub degraded: DegradedStores,
    /// Startup job results.
    ///
    /// Accumulating field.
    pub jobs: StartupJobs,
}

#[cfg(test)]
mod tests;
