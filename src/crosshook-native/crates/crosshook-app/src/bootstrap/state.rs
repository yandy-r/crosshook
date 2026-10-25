//! Live startup state and the cloneable snapshot handle.
//!
//! [`StartupState`] accumulates job results; [`StartupHandle`] is the
//! `Arc`-backed reader/scheduler handle. Job results are stored **before**
//! the matching `AppEvent::StartupJobFinished` is published (state-then-
//! publish), so a snapshot taken after the event always contains the result.
//!
//! Frozen vs live fields: `flatpak`, `degraded` and `jobs` are fixed at
//! bootstrap (jobs then accumulate terminal states); `auto_load_profile` and
//! `onboarding` are **live** — every [`StartupHandle::snapshot`] recomputes
//! them from the settings store and profile store so a webview reload after
//! dismissing onboarding or switching profiles never replays a stale answer.

use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;

use crate::events::StartupJobKind;
use crate::services::health::EnrichedHealthSummary;
use crate::services::BackgroundProtectionState;

use super::reconcile::{derive_onboarding, resolve_auto_load_profile};
use super::report::{
    DegradedStores, FlatpakStartup, JobSnapshotKind, JobState, OfflineReadinessScanSummary,
    OnboardingStartup, StartupJobs, StartupReport, UmuRefreshSummary, VersionScanSummary,
};
use super::stores::Stores;

/// Mutable startup report state behind [`StartupHandle`].
pub(crate) struct StartupState {
    flatpak: FlatpakStartup,
    degraded: DegradedStores,
    jobs: StartupJobs,
}

impl StartupState {
    fn snapshot_with(
        &self,
        auto_load_profile: Option<String>,
        onboarding: OnboardingStartup,
    ) -> StartupReport {
        StartupReport {
            auto_load_profile,
            onboarding,
            flatpak: self.flatpak.clone(),
            degraded: self.degraded.clone(),
            jobs: self.jobs.clone(),
        }
    }
}

/// Interior-mutable shared startup state.
struct Inner {
    stores: Stores,
    state: Mutex<StartupState>,
    /// Retained terminal kinds; `send_replace` keeps updates even with zero
    /// receivers so [`StartupHandle::wait_finished`] never hangs.
    job_watch: watch::Sender<JobKinds>,
}

/// Cloneable startup report handle. Holds the store bundle but **no** `App`
/// clone, so bridges and jobs can retain it without keeping the app alive.
#[derive(Clone)]
pub struct StartupHandle {
    inner: Arc<Inner>,
}

impl StartupHandle {
    /// Builds the handle from the initial (all-pending) report and the store
    /// bundle used for live field recomputation.
    pub(crate) fn new(stores: Stores, report: StartupReport) -> Self {
        let state = StartupState {
            flatpak: report.flatpak,
            degraded: report.degraded,
            jobs: report.jobs,
        };
        let job_watch = watch::Sender::new(JobKinds::from(&state.jobs));
        Self {
            inner: Arc::new(Inner {
                stores,
                state: Mutex::new(state),
                job_watch,
            }),
        }
    }

    /// Live, idempotent snapshot. Settings-derived fields
    /// (`auto_load_profile`, `onboarding`) are recomputed on every call;
    /// job results and bootstrap-fixed fields come from the accumulated state.
    /// An unreadable settings file degrades to defaults (logged), matching the
    /// Phase-B degraded-not-fatal contract.
    pub fn snapshot(&self) -> StartupReport {
        let settings = self.inner.stores.settings.load().unwrap_or_else(|error| {
            tracing::warn!(%error, "startup snapshot: settings unreadable; using defaults");
            Default::default()
        });
        let profiles = self.inner.stores.profiles.list().unwrap_or_else(|error| {
            tracing::warn!(%error, "startup snapshot: failed to list profiles; assuming none");
            Vec::new()
        });
        let auto_load_profile = resolve_auto_load_profile(&settings, &profiles);
        let onboarding = derive_onboarding(&settings, &profiles);
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .snapshot_with(auto_load_profile, onboarding)
    }

    /// Clones only retained job results; performs no settings or profile I/O.
    pub fn jobs_snapshot(&self) -> StartupJobs {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .jobs
            .clone()
    }

    /// Resolves when `job` reaches a terminal state (`complete`, `failed` or
    /// `skipped`) and returns that terminal kind. No sleeps; driven by the
    /// retained watch channel, so calling after completion returns immediately.
    pub async fn wait_finished(&self, job: StartupJobKind) -> JobSnapshotKind {
        let mut rx = self.inner.job_watch.subscribe();
        // Subscribe first, then check: an update between subscribe and the
        // check is retained by the watch channel and observed by `changed()`.
        if let Some(kind) = terminal_kind(job, &rx.borrow_and_update()) {
            return kind;
        }
        loop {
            if rx.changed().await.is_err() {
                // Sender dropped: fall back to the last retained value.
                return terminal_kind(job, &rx.borrow()).unwrap_or(JobSnapshotKind::Pending);
            }
            if let Some(kind) = terminal_kind(job, &rx.borrow_and_update()) {
                return kind;
            }
        }
    }

    /// Replaces one job's state, then retains the update in the watch
    /// channel. The caller publishes the bus event **after** this returns.
    pub(crate) fn set_job_state(&self, next: JobUpdate) -> StartupJobKind {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let job = apply(&mut state.jobs, next);
        self.inner
            .job_watch
            .send_replace(JobKinds::from(&state.jobs));
        job
    }
}

/// Payload-free retained watch value: updates never clone result vectors.
#[derive(Clone, Copy)]
struct JobKinds {
    health: JobSnapshotKind,
    offline_readiness: JobSnapshotKind,
    version_scan: JobSnapshotKind,
    umu_refresh: JobSnapshotKind,
    background_portal: JobSnapshotKind,
}

impl From<&StartupJobs> for JobKinds {
    fn from(jobs: &StartupJobs) -> Self {
        Self {
            health: jobs.health.kind(),
            offline_readiness: jobs.offline_readiness.kind(),
            version_scan: jobs.version_scan.kind(),
            umu_refresh: jobs.umu_refresh.kind(),
            background_portal: jobs.background_portal.kind(),
        }
    }
}

fn terminal_kind(job: StartupJobKind, jobs: &JobKinds) -> Option<JobSnapshotKind> {
    let kind = match job {
        StartupJobKind::HealthBatch => jobs.health,
        StartupJobKind::OfflineReadiness => jobs.offline_readiness,
        StartupJobKind::VersionScan => jobs.version_scan,
        StartupJobKind::UmuRefresh => jobs.umu_refresh,
        StartupJobKind::BackgroundPortal => jobs.background_portal,
    };
    match kind {
        JobSnapshotKind::Pending | JobSnapshotKind::Running => None,
        JobSnapshotKind::Complete | JobSnapshotKind::Failed | JobSnapshotKind::Skipped => {
            Some(kind)
        }
    }
}

/// Type-erased job state transition so `StartupState` stays independent of
/// the per-job payload types.
pub(crate) enum JobUpdate {
    Health(JobState<Arc<EnrichedHealthSummary>>),
    OfflineReadiness(JobState<OfflineReadinessScanSummary>),
    VersionScan(JobState<VersionScanSummary>),
    UmuRefresh(JobState<UmuRefreshSummary>),
    BackgroundPortal(JobState<BackgroundProtectionState>),
}

impl JobUpdate {
    /// `Running` marker for `job` (no payload).
    pub(crate) fn running(job: StartupJobKind) -> Self {
        match job {
            StartupJobKind::HealthBatch => Self::Health(JobState::Running),
            StartupJobKind::OfflineReadiness => Self::OfflineReadiness(JobState::Running),
            StartupJobKind::VersionScan => Self::VersionScan(JobState::Running),
            StartupJobKind::UmuRefresh => Self::UmuRefresh(JobState::Running),
            StartupJobKind::BackgroundPortal => Self::BackgroundPortal(JobState::Running),
        }
    }

    /// Terminal `skipped` marker used when a job is cancelled before running,
    /// so `wait_finished` always resolves instead of hanging on `pending`.
    pub(crate) fn cancelled(job: StartupJobKind) -> Self {
        let reason = "cancelled".to_string();
        match job {
            StartupJobKind::HealthBatch => Self::Health(JobState::Skipped { reason }),
            StartupJobKind::OfflineReadiness => {
                Self::OfflineReadiness(JobState::Skipped { reason })
            }
            StartupJobKind::VersionScan => Self::VersionScan(JobState::Skipped { reason }),
            StartupJobKind::UmuRefresh => Self::UmuRefresh(JobState::Skipped { reason }),
            StartupJobKind::BackgroundPortal => {
                Self::BackgroundPortal(JobState::Skipped { reason })
            }
        }
    }

    /// Terminal `failed` marker used when the job task could not be spawned.
    pub(crate) fn start_failed(job: StartupJobKind, message: String) -> Self {
        match job {
            StartupJobKind::HealthBatch => Self::Health(JobState::Failed { message }),
            StartupJobKind::OfflineReadiness => {
                Self::OfflineReadiness(JobState::Failed { message })
            }
            StartupJobKind::VersionScan => Self::VersionScan(JobState::Failed { message }),
            StartupJobKind::UmuRefresh => Self::UmuRefresh(JobState::Failed { message }),
            StartupJobKind::BackgroundPortal => {
                Self::BackgroundPortal(JobState::Failed { message })
            }
        }
    }

    pub(crate) fn health_complete(data: EnrichedHealthSummary) -> Self {
        Self::Health(JobState::Complete {
            data: Arc::new(data),
        })
    }

    pub(crate) fn health_failed(message: String) -> Self {
        Self::Health(JobState::Failed { message })
    }

    pub(crate) fn offline_readiness_complete(data: OfflineReadinessScanSummary) -> Self {
        Self::OfflineReadiness(JobState::Complete { data })
    }

    pub(crate) fn offline_readiness_failed(message: String) -> Self {
        Self::OfflineReadiness(JobState::Failed { message })
    }

    pub(crate) fn version_scan_complete(data: VersionScanSummary) -> Self {
        Self::VersionScan(JobState::Complete { data })
    }

    pub(crate) fn version_scan_failed(message: String) -> Self {
        Self::VersionScan(JobState::Failed { message })
    }

    pub(crate) fn umu_refresh_complete(data: UmuRefreshSummary) -> Self {
        Self::UmuRefresh(JobState::Complete { data })
    }

    pub(crate) fn umu_refresh_failed(message: String) -> Self {
        Self::UmuRefresh(JobState::Failed { message })
    }

    pub(crate) fn umu_refresh_skipped(reason: String) -> Self {
        Self::UmuRefresh(JobState::Skipped { reason })
    }

    pub(crate) fn background_portal_complete(data: BackgroundProtectionState) -> Self {
        Self::BackgroundPortal(JobState::Complete { data })
    }
}

fn apply(jobs: &mut StartupJobs, next: JobUpdate) -> StartupJobKind {
    match next {
        JobUpdate::Health(state) => {
            jobs.health = state;
            StartupJobKind::HealthBatch
        }
        JobUpdate::OfflineReadiness(state) => {
            jobs.offline_readiness = state;
            StartupJobKind::OfflineReadiness
        }
        JobUpdate::VersionScan(state) => {
            jobs.version_scan = state;
            StartupJobKind::VersionScan
        }
        JobUpdate::UmuRefresh(state) => {
            jobs.umu_refresh = state;
            StartupJobKind::UmuRefresh
        }
        JobUpdate::BackgroundPortal(state) => {
            jobs.background_portal = state;
            StartupJobKind::BackgroundPortal
        }
    }
}

#[cfg(test)]
mod tests;
