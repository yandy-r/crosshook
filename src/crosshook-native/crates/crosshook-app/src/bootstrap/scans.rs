//! Startup job scheduling: the health → offline-readiness notice → version
//! scan sequence (run by [`StartupScansService`]) in one job, plus the
//! independent umu refresh and background portal requests.
//!
//! Contract rules honored here:
//!
//! - **State-then-publish**: every job writes its terminal state into
//!   [`StartupHandle`] first, then publishes `AppEvent::StartupJobFinished`,
//!   so a snapshot read off the event always contains the result.
//! - **No hangs**: `spawn_blocking` join errors (panic/cancel) are logged
//!   raw and stored as `failed` with the sanitized [`AppError`] message.
//!   The sequence continues; a step cancelled before it starts is
//!   `skipped`; a job that fails to *schedule* lands `failed`. No
//!   `pending` state survives a finished or unstartable job.
//! - **No owner retention**: closures capture [`StartupHandle`], [`EventBus`]
//!   and store/service clones — never an `App` clone.

use crosshook_core::umu_database;

use crate::bootstrap::options::UmuRefresh;
use crate::bootstrap::report::UmuRefreshSummary;
use crate::bootstrap::state::{JobUpdate, StartupHandle};
use crate::bootstrap::stores::{Services, Stores};
use crate::bootstrap::StartupJobKind;
use crate::events::EventBus;
use crate::jobs::{JobSpec, Outcome};
use crate::services::startup_scans::{
    finish, finish_skipped_cancelled, finish_start_failed, mark_running,
};
use crate::services::StartupScansService;
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

fn schedule_scans(
    app: &App,
    startup: StartupHandle,
    stores: &Stores,
    services: &Services,
    events: EventBus,
) {
    let scans = StartupScansService::new(
        services.health.clone(),
        stores.profiles.clone(),
        stores.metadata.clone(),
        events.clone(),
    );
    // Cloned for the post-`start` error path: the closure takes its own.
    let task_startup = startup.clone();
    let result = app.jobs().start(JobSpec::new("startup-scans"), move |ctx| {
        let startup = task_startup;
        async move { scans.run(&ctx, &startup).await }
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
