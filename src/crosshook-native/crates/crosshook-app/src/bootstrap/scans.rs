//! Startup job scheduling: the health → offline-readiness notice → version
//! scan sequence (run by [`StartupScansService`]) in one job, plus the
//! independent umu refresh, background portal requests and prefix
//! Windows-version journal recovery.
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

use crate::bootstrap::options::{PrefixJournalRecovery, UmuRefresh};
use crate::bootstrap::report::UmuRefreshSummary;
use crate::bootstrap::state::{JobUpdate, StartupHandle};
use crate::bootstrap::stores::{Services, Stores};
use crate::bootstrap::StartupJobKind;
use crate::events::EventBus;
use crate::jobs::{JobContext, JobSpec, Outcome};
use crate::services::startup_scans::{
    finish, finish_skipped_cancelled, finish_start_failed, mark_running,
};
use crate::services::StartupScansService;
use crate::App;

/// Job kind of the startup prefix Windows-version journal recovery job.
const PREFIX_JOURNAL_RECOVERY_JOB_KIND: &str = "prefix-journal-recovery";

/// Schedules the startup jobs on the app runtime.
///
/// `startup-scans` runs health → offline-readiness notice → version scan
/// sequentially (replacing the old 350/500/2000 ms staggering with
/// deterministic order); one failing step does not stop the next.
/// `startup-umu-refresh` and `startup-background-portal` are independent
/// async jobs. `prefix-journal-recovery` is an untracked background job (not a
/// [`StartupJobKind`]) governed by `recovery`.
pub(crate) fn schedule(
    app: &App,
    startup: StartupHandle,
    stores: &Stores,
    services: &Services,
    umu: UmuRefresh,
    recovery: PrefixJournalRecovery,
) {
    let events = app.events().clone();

    schedule_scans(app, startup.clone(), stores, services, events.clone());
    schedule_umu(app, startup.clone(), services, umu, events.clone());
    schedule_background_portal(app, startup, services, events);
    schedule_prefix_journal_recovery(app, stores, services, recovery);
}

/// Schedules the `prefix-journal-recovery` job: retries `pending` prefix
/// Windows-version journal rows through [`PrefixDepsService`], each under its
/// prefix key (busy and in-use prefixes are left `pending`; `failed` rows wait
/// for an explicit repair). Skipped under [`PrefixJournalRecovery::Skip`] or
/// when the metadata store is unavailable. The job carries no key of its own.
///
/// [`PrefixDepsService`]: crate::services::PrefixDepsService
fn schedule_prefix_journal_recovery(
    app: &App,
    stores: &Stores,
    services: &Services,
    policy: PrefixJournalRecovery,
) {
    if let PrefixJournalRecovery::Skip { reason } = policy {
        tracing::info!("prefix journal startup recovery skipped ({reason})");
        return;
    }
    if !stores.metadata.is_available() {
        tracing::info!("prefix journal startup recovery skipped (metadata store unavailable)");
        return;
    }
    let prefix_deps = services.prefix_deps.clone();
    let result = app.jobs().start(
        JobSpec::new(PREFIX_JOURNAL_RECOVERY_JOB_KIND),
        move |ctx: JobContext| async move {
            if ctx.is_cancelled() {
                return Outcome::Cancelled;
            }
            let results = tokio::select! {
                biased;
                () = ctx.cancelled() => return Outcome::Cancelled,
                results = prefix_deps.recover_on_startup() => results,
            };
            for (prefix, result) in &results {
                tracing::info!(prefix = %prefix, ?result, "prefix journal startup recovery");
            }
            Outcome::Succeeded
        },
    );
    if let Err(error) = result {
        tracing::warn!(%error, "prefix-journal-recovery job could not be scheduled");
    }
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

fn schedule_umu(
    app: &App,
    startup: StartupHandle,
    services: &Services,
    umu: UmuRefresh,
    events: EventBus,
) {
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
            let umu_service = services.umu.clone();
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
                        let update = match umu_service.refresh().await {
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
                                JobUpdate::umu_refresh_failed(error.message().to_string())
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

#[cfg(test)]
mod tests {
    use crosshook_core::metadata::MetadataStore;
    use crosshook_core::settings::AppSettingsData;
    use tempfile::tempdir;

    use super::*;
    use crate::bootstrap::options::StoreRoots;
    use crate::bootstrap::stores::{init_remaining, init_settings};

    fn stores_in(dir: &std::path::Path) -> Stores {
        let roots = StoreRoots::Explicit {
            config_dir: dir.join("config"),
            data_dir: dir.join("data"),
        };
        let settings = init_settings(&roots).unwrap();
        let (stores, metadata_error) =
            init_remaining(&roots, &settings, &AppSettingsData::default()).unwrap();
        assert_eq!(metadata_error, None);
        stores
    }

    fn services_for(app: &App, stores: &Stores) -> Services {
        Services::new(
            stores,
            app.events().clone(),
            app.runtime_handle(),
            app.jobs().clone(),
        )
    }

    #[tokio::test]
    async fn skip_policy_schedules_no_recovery_job() {
        let temp = tempdir().unwrap();
        let app = App::with_handle(tokio::runtime::Handle::current());
        let stores = stores_in(temp.path());
        let services = services_for(&app, &stores);

        schedule_prefix_journal_recovery(
            &app,
            &stores,
            &services,
            PrefixJournalRecovery::Skip { reason: "test" },
        );

        assert_eq!(app.jobs().job_counts(), (0, 0));
    }

    #[tokio::test]
    async fn unavailable_metadata_schedules_no_recovery_job() {
        let temp = tempdir().unwrap();
        let app = App::with_handle(tokio::runtime::Handle::current());
        let stores = Stores {
            metadata: MetadataStore::disabled(),
            ..stores_in(temp.path())
        };
        let services = services_for(&app, &stores);

        schedule_prefix_journal_recovery(&app, &stores, &services, PrefixJournalRecovery::Enabled);

        assert_eq!(app.jobs().job_counts(), (0, 0));
    }

    #[tokio::test]
    async fn enabled_policy_schedules_one_recovery_job() {
        let temp = tempdir().unwrap();
        let app = App::with_handle(tokio::runtime::Handle::current());
        let stores = stores_in(temp.path());
        let services = services_for(&app, &stores);

        schedule_prefix_journal_recovery(&app, &stores, &services, PrefixJournalRecovery::Enabled);

        // The current-thread test runtime has not polled the job yet, so it
        // is still running; dropping `app` cancels it.
        assert_eq!(app.jobs().job_counts(), (1, 0));
    }
}
