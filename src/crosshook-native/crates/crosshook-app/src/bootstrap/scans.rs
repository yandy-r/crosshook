//! Startup scan jobs: health → offline-readiness notice → version scan, run
//! sequentially in one job, plus the independent umu refresh and background
//! portal requests.
//!
//! Contract rules honored here:
//!
//! - **State-then-publish**: every job writes its terminal state into
//!   [`StartupHandle`] first, then publishes `AppEvent::StartupJobFinished`,
//!   so a snapshot read off the event always contains the result.
//! - **No hangs**: `spawn_blocking` join errors (panic/cancel) map to
//!   `failed` and the sequence continues; a step cancelled before it starts
//!   is `skipped`; a job that fails to *schedule* lands `failed`. No
//!   `pending` state survives a finished or unstartable job.
//! - **No owner retention**: closures capture [`StartupHandle`], [`EventBus`]
//!   and store/service clones — never an `App` clone.

use crosshook_core::profile::ProfileStore;
use crosshook_core::umu_database;

use crate::bootstrap::options::UmuRefresh;
use crate::bootstrap::report::{OfflineReadinessScanSummary, UmuRefreshSummary};
use crate::bootstrap::state::{JobUpdate, StartupHandle};
use crate::bootstrap::stores::{Services, Stores};
use crate::bootstrap::StartupJobKind;
use crate::events::{AppEvent, EventBus};
use crate::jobs::{JobSpec, Outcome};
use crate::App;

/// Schedules the three startup jobs on the app runtime.
///
/// `startup-scans` runs health → offline-readiness notice → version scan
/// sequentially (replacing the old 350/500/2000 ms staggering with
/// deterministic order); one failing step does not stop the next.
/// `startup-umu-refresh` and `startup-background-portal` are independent
/// async jobs.
pub(crate) fn schedule(
    app: &App,
    startup: StartupHandle,
    stores: &Stores,
    services: &Services,
    umu: UmuRefresh,
) {
    let events = app.events().clone();

    schedule_scans(app, startup.clone(), stores, services, events.clone());
    schedule_umu(app, startup.clone(), umu, events.clone());
    schedule_background_portal(app, startup, services, events);
}

/// Marks a job running. Only terminal states are bridged to the shell.
fn mark_running(startup: &StartupHandle, job: StartupJobKind) {
    startup.set_job_state(JobUpdate::running(job));
}

/// Stores the terminal state, then publishes the completion event.
fn finish(startup: &StartupHandle, events: &EventBus, update: JobUpdate) {
    let job = startup.set_job_state(update);
    events.publish(AppEvent::StartupJobFinished { job });
}

/// Terminal `skipped` marker for a job cancelled before it ran, so
/// `wait_finished` always resolves instead of hanging on `pending`.
fn finish_skipped_cancelled(startup: &StartupHandle, events: &EventBus, job: StartupJobKind) {
    finish(startup, events, JobUpdate::cancelled(job));
}

/// A job that could not even be spawned lands `failed`, never `pending`.
fn finish_start_failed(
    startup: &StartupHandle,
    events: &EventBus,
    job: StartupJobKind,
    message: String,
) {
    finish(startup, events, JobUpdate::start_failed(job, message));
}

fn schedule_scans(
    app: &App,
    startup: StartupHandle,
    stores: &Stores,
    services: &Services,
    events: EventBus,
) {
    let health = services.health.clone();
    let profiles = stores.profiles.clone();
    let metadata = stores.metadata.clone();
    // Cloned for the post-`start` error path: the closure takes its own.
    let task_startup = startup.clone();
    let task_events = events.clone();
    let result = app.jobs().start(JobSpec::new("startup-scans"), move |ctx| {
        let startup = task_startup;
        let events = task_events;
        async move {
            // Step 1: enriched health batch (async; blocking I/O runs on the
            // service's blocking pool).
            if ctx.is_cancelled() {
                finish_skipped_cancelled(&startup, &events, StartupJobKind::HealthBatch);
                finish_skipped_cancelled(&startup, &events, StartupJobKind::OfflineReadiness);
                finish_skipped_cancelled(&startup, &events, StartupJobKind::VersionScan);
                return Outcome::Cancelled;
            }
            mark_running(&startup, StartupJobKind::HealthBatch);
            let health_result = health.batch_summary().await;
            let health_total = health_result
                .as_ref()
                .map(|summary| summary.total_count)
                .unwrap_or(0);
            let update = match health_result {
                Ok(summary) => JobUpdate::health_complete(summary),
                Err(error) => JobUpdate::health_failed(error.to_string()),
            };
            finish(&startup, &events, update);

            // Step 2: offline-readiness notice. Reports the number of
            // profiles the scan covers: the health batch total when it ran,
            // otherwise a fresh profile-list count.
            if ctx.is_cancelled() {
                finish_skipped_cancelled(&startup, &events, StartupJobKind::OfflineReadiness);
                finish_skipped_cancelled(&startup, &events, StartupJobKind::VersionScan);
                return Outcome::Cancelled;
            }
            mark_running(&startup, StartupJobKind::OfflineReadiness);
            let profiles_for_scan = profiles.clone();
            let update = match tokio::task::spawn_blocking(move || {
                offline_readiness_notice(&profiles_for_scan, health_total)
            })
            .await
            {
                Ok(summary) => JobUpdate::offline_readiness_complete(summary),
                Err(join) => JobUpdate::offline_readiness_failed(format!(
                    "offline readiness scan task failed: {join}"
                )),
            };
            finish(&startup, &events, update);

            // Step 3: version correlation scan.
            if ctx.is_cancelled() {
                finish_skipped_cancelled(&startup, &events, StartupJobKind::VersionScan);
                return Outcome::Cancelled;
            }
            mark_running(&startup, StartupJobKind::VersionScan);
            let update = match tokio::task::spawn_blocking(move || {
                super::version_scan::run(&profiles, &metadata)
            })
            .await
            {
                Ok(summary) => JobUpdate::version_scan_complete(summary),
                Err(join) => {
                    JobUpdate::version_scan_failed(format!("version scan task failed: {join}"))
                }
            };
            finish(&startup, &events, update);

            Outcome::Succeeded
        }
    });
    if let Err(error) = result {
        tracing::warn!(%error, "startup-scans job could not be scheduled");
        let message = format!("job could not be scheduled: {error}");
        for job in [
            StartupJobKind::HealthBatch,
            StartupJobKind::OfflineReadiness,
            StartupJobKind::VersionScan,
        ] {
            finish_start_failed(&startup, &events, job, message.clone());
        }
    }
}

/// Builds the offline-readiness notice payload. Prefers the health batch's
/// total (already computed this run); falls back to listing profiles when
/// health failed.
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

fn schedule_umu(app: &App, startup: StartupHandle, umu: UmuRefresh, events: EventBus) {
    match umu {
        UmuRefresh::Skip { reason } => {
            // Skipped is terminal: store-then-publish so `wait_finished`
            // resolves immediately and the report carries the reason.
            tracing::info!("umu-database startup refresh skipped ({reason})");
            finish(
                &startup,
                &events,
                JobUpdate::umu_refresh_skipped(reason.to_string()),
            );
        }
        UmuRefresh::Enabled => {
            let task_startup = startup.clone();
            let task_events = events.clone();
            let result = app
                .jobs()
                .start(JobSpec::new("startup-umu-refresh"), move |ctx| {
                    let startup = task_startup;
                    let events = task_events;
                    async move {
                        if ctx.is_cancelled() {
                            finish_skipped_cancelled(&startup, &events, StartupJobKind::UmuRefresh);
                            return Outcome::Cancelled;
                        }
                        mark_running(&startup, StartupJobKind::UmuRefresh);
                        let update = match umu_database::refresh_umu_database().await {
                            Ok(status) => {
                                tracing::info!(?status, "umu-database startup refresh complete");
                                JobUpdate::umu_refresh_complete(UmuRefreshSummary {
                                    refreshed: status.refreshed,
                                    cached_at: status.cached_at,
                                    reason: status.reason,
                                })
                            }
                            Err(error) => {
                                tracing::warn!(%error, "umu-database startup refresh failed");
                                JobUpdate::umu_refresh_failed(crate::presenter::sanitize_message(
                                    &error.to_string(),
                                ))
                            }
                        };
                        finish(&startup, &events, update);
                        Outcome::Succeeded
                    }
                });
            if let Err(error) = result {
                tracing::warn!(%error, "startup-umu-refresh job could not be scheduled");
                finish_start_failed(
                    &startup,
                    &events,
                    StartupJobKind::UmuRefresh,
                    format!("job could not be scheduled: {error}"),
                );
            }
        }
    }
}

fn schedule_background_portal(
    app: &App,
    startup: StartupHandle,
    services: &Services,
    events: EventBus,
) {
    let holder = services.background.clone();
    let task_startup = startup.clone();
    let task_events = events.clone();
    let result = app
        .jobs()
        .start(JobSpec::new("startup-background-portal"), move |ctx| {
            let startup = task_startup;
            let events = task_events;
            async move {
                if ctx.is_cancelled() {
                    finish_skipped_cancelled(&startup, &events, StartupJobKind::BackgroundPortal);
                    return Outcome::Cancelled;
                }
                mark_running(&startup, StartupJobKind::BackgroundPortal);
                holder
                    .request_and_store(&crate::services::XdgBackgroundPortal)
                    .await;
                finish(
                    &startup,
                    &events,
                    JobUpdate::background_portal_complete(holder.protection_state()),
                );
                Outcome::Succeeded
            }
        });
    if let Err(error) = result {
        tracing::warn!(%error, "startup-background-portal job could not be scheduled");
        finish_start_failed(
            &startup,
            &events,
            StartupJobKind::BackgroundPortal,
            format!("job could not be scheduled: {error}"),
        );
    }
}
