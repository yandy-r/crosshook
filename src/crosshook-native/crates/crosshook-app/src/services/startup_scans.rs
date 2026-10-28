//! Startup scan sequence: health batch → offline-readiness notice → version
//! scan, run sequentially inside one supervised `startup-scans` job.
//!
//! Contract rules honored here:
//!
//! - **State-then-publish**: each step writes its terminal state into
//!   [`StartupHandle`], publishes [`AppEvent::StartupJobFinished`], and only
//!   then — on success — publishes the matching named event
//!   ([`AppEvent::HealthScanCompleted`], [`AppEvent::OfflineReadinessScanCompleted`],
//!   [`AppEvent::VersionScanCompleted`]). A snapshot read off any of these
//!   events already contains the result.
//! - **No hangs**: `spawn_blocking` join errors (panic/cancel) are logged raw
//!   and stored as `failed` with the sanitized [`AppError`] message. The
//!   sequence continues; a step cancelled before it starts is `skipped`.
//! - **No owner retention**: the service holds store/service clones and the
//!   [`EventBus`] only — never an `App` clone.

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;

use super::HealthService;
use crate::bootstrap::state::{JobUpdate, StartupHandle};
use crate::bootstrap::{version_scan, OfflineReadinessScanSummary, StartupJobKind};
use crate::events::{AppEvent, EventBus};
use crate::jobs::{JobContext, Outcome};
use crate::{AppError, AppErrorKind};

/// Marks a job running. Only terminal states are bridged to the shell.
pub(crate) fn mark_running(startup: &StartupHandle, job: StartupJobKind) {
    startup.set_job_state(JobUpdate::running(job));
}

/// Stores the terminal state, then publishes the completion event.
pub(crate) fn finish(startup: &StartupHandle, events: &EventBus, update: JobUpdate) {
    let job = startup.set_job_state(update);
    events.publish(AppEvent::StartupJobFinished { job });
}

/// Terminal `skipped` marker for a job cancelled before it ran, so
/// `wait_finished` always resolves instead of hanging on `pending`.
pub(crate) fn finish_skipped_cancelled(
    startup: &StartupHandle,
    events: &EventBus,
    job: StartupJobKind,
) {
    finish(startup, events, JobUpdate::cancelled(job));
}

/// A job that could not even be spawned lands `failed`, never `pending`.
pub(crate) fn finish_start_failed(
    startup: &StartupHandle,
    events: &EventBus,
    job: StartupJobKind,
    message: String,
) {
    finish(startup, events, JobUpdate::start_failed(job, message));
}

/// Runs the startup health → offline-readiness → version scan sequence.
#[derive(Clone)]
pub struct StartupScansService {
    health: HealthService,
    profiles: ProfileStore,
    metadata: MetadataStore,
    events: EventBus,
}

impl StartupScansService {
    /// Builds the service over the shared health service, stores and the app
    /// event bus that carries the named completion events.
    pub fn new(
        health: HealthService,
        profiles: ProfileStore,
        metadata: MetadataStore,
        events: EventBus,
    ) -> Self {
        Self {
            health,
            profiles,
            metadata,
            events,
        }
    }

    /// Runs the sequence as the body of the `startup-scans` job.
    ///
    /// One failing step does not stop the next. When `ctx` is cancelled before
    /// a step starts, that step and every later one land `skipped` and the
    /// job resolves to [`Outcome::Cancelled`].
    pub(crate) async fn run(&self, ctx: &JobContext, startup: &StartupHandle) -> Outcome {
        let events = &self.events;

        // Step 1: enriched health batch (itself a supervised blocking job).
        if ctx.is_cancelled() {
            finish_skipped_cancelled(startup, events, StartupJobKind::HealthBatch);
            finish_skipped_cancelled(startup, events, StartupJobKind::OfflineReadiness);
            finish_skipped_cancelled(startup, events, StartupJobKind::VersionScan);
            return Outcome::Cancelled;
        }
        mark_running(startup, StartupJobKind::HealthBatch);
        let health_total = match self.health.batch_summary().await {
            Ok(summary) => {
                let counts = AppEvent::HealthScanCompleted {
                    total: summary.total_count,
                    healthy: summary.healthy_count,
                    stale: summary.stale_count,
                    broken: summary.broken_count,
                };
                let total = summary.total_count;
                finish(startup, events, JobUpdate::health_complete(summary));
                events.publish(counts);
                total
            }
            Err(error) if error.kind() == AppErrorKind::Cancelled => {
                // App shutdown cancelled the nested batch job: not a failure.
                finish_skipped_cancelled(startup, events, StartupJobKind::HealthBatch);
                finish_skipped_cancelled(startup, events, StartupJobKind::OfflineReadiness);
                finish_skipped_cancelled(startup, events, StartupJobKind::VersionScan);
                return Outcome::Cancelled;
            }
            Err(error) => {
                finish(startup, events, JobUpdate::health_failed(error.to_string()));
                0
            }
        };

        // Step 2: offline-readiness notice. Reports the number of profiles
        // the scan covers: the health batch total when it ran, otherwise a
        // fresh profile-list count.
        if ctx.is_cancelled() {
            finish_skipped_cancelled(startup, events, StartupJobKind::OfflineReadiness);
            finish_skipped_cancelled(startup, events, StartupJobKind::VersionScan);
            return Outcome::Cancelled;
        }
        mark_running(startup, StartupJobKind::OfflineReadiness);
        let profiles = self.profiles.clone();
        match tokio::task::spawn_blocking(move || offline_readiness_notice(&profiles, health_total))
            .await
        {
            Ok(summary) => {
                let total_profiles = summary.total_profiles;
                finish(
                    startup,
                    events,
                    JobUpdate::offline_readiness_complete(summary),
                );
                events.publish(AppEvent::OfflineReadinessScanCompleted { total_profiles });
            }
            Err(join) => {
                tracing::warn!(%join, "offline readiness scan task failed");
                finish(
                    startup,
                    events,
                    JobUpdate::offline_readiness_failed(AppError::from(join).to_string()),
                );
            }
        }

        // Step 3: version correlation scan.
        if ctx.is_cancelled() {
            finish_skipped_cancelled(startup, events, StartupJobKind::VersionScan);
            return Outcome::Cancelled;
        }
        mark_running(startup, StartupJobKind::VersionScan);
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        match tokio::task::spawn_blocking(move || version_scan::run(&profiles, &metadata)).await {
            Ok(summary) => {
                let (scanned, mismatches) = (summary.scanned, summary.mismatches);
                finish(startup, events, JobUpdate::version_scan_complete(summary));
                events.publish(AppEvent::VersionScanCompleted {
                    scanned,
                    mismatches,
                });
            }
            Err(join) => {
                tracing::warn!(%join, "version scan task failed");
                finish(
                    startup,
                    events,
                    JobUpdate::version_scan_failed(AppError::from(join).to_string()),
                );
            }
        }

        Outcome::Succeeded
    }
}

/// Builds the offline-readiness notice payload. Prefers the health batch's
/// total (already computed this run); falls back to listing profiles when
/// health failed or found nothing.
fn offline_readiness_notice(
    profiles: &ProfileStore,
    health_total: usize,
) -> OfflineReadinessScanSummary {
    let total_profiles = if health_total > 0 {
        health_total
    } else {
        profiles
            .list()
            .map(|names| names.len())
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "offline readiness scan: failed to list profiles");
                0
            })
    };
    OfflineReadinessScanSummary { total_profiles }
}

#[cfg(test)]
mod tests;
