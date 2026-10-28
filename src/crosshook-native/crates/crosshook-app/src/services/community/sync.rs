use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crosshook_core::community::CommunityTapSyncResult;

use super::persist::{check_cancelled, finalize_sync_results};
use super::{CommunityService, COMMUNITY_SYNC_JOB_KIND};
use crate::jobs::{JobContext, JobEventKind, JobHandle, JobId, JobSpec, Outcome};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl CommunityService {
    /// Starts a cancellable sync; simultaneous requests queue instead of conflicting.
    pub fn start_sync(&self) -> Result<JobHandle, AppError> {
        self.start_sync_inner().map(|(handle, _)| handle)
    }

    fn start_sync_inner(&self) -> Result<(JobHandle, Arc<AtomicBool>), AppError> {
        let service = self.clone();
        let abandoned = Arc::new(AtomicBool::new(false));
        let job_abandoned = abandoned.clone();
        let handle = self.jobs.start(
            JobSpec::new(COMMUNITY_SYNC_JOB_KIND),
            move |ctx| async move {
                match service.run_sync(&ctx).await {
                    Ok(result) => {
                        let mut results = match service.sync_results.lock() {
                            Ok(results) => results,
                            Err(_) => return Outcome::Failed(result_slot_error()),
                        };
                        if ctx.is_cancelled() {
                            return Outcome::Cancelled;
                        }
                        if !job_abandoned.load(Ordering::Acquire) {
                            results.insert(ctx.id().clone(), result);
                        }
                        Outcome::Succeeded
                    }
                    Err(error) if error.kind() == AppErrorKind::Cancelled => Outcome::Cancelled,
                    Err(error) => Outcome::Failed(error),
                }
            },
        )?;
        Ok((handle, abandoned))
    }

    /// Waits for synchronization, takes its result and reaps the job on every outcome.
    /// Dropping this waiter abandons the result and arranges terminal job cleanup,
    /// but it does **not** cancel the job: the sync keeps running in the
    /// background to completion. Cancel explicitly through the [`JobHandle`]
    /// from [`Self::start_sync`] or the job registry.
    pub async fn sync(&self) -> Result<Vec<CommunityTapSyncResult>, AppError> {
        let (handle, abandoned) = self.start_sync_inner()?;
        let id = handle.id().clone();
        let mut events = handle.into_events();
        let _guard = SyncResultGuard {
            service: self.clone(),
            id: id.clone(),
            abandoned,
        };
        while let Some(event) = events.recv().await {
            if let JobEventKind::Finished(outcome) = event.kind {
                return match outcome {
                    Outcome::Succeeded => self.sync_result(&id)?.ok_or_else(result_slot_error),
                    Outcome::Failed(error) => Err(error),
                    Outcome::Cancelled => Err(AppError::cancelled()),
                };
            }
        }
        Err(AppError::cancelled())
    }

    /// Takes a successful job's result once, even after its registry outcome is
    /// evicted; unknown ids without a result return `NotFound`.
    pub fn sync_result(&self, id: &JobId) -> Result<Option<Vec<CommunityTapSyncResult>>, AppError> {
        if let Some(result) = self
            .sync_results
            .lock()
            .map_err(|_| result_slot_error())?
            .remove(id)
        {
            return Ok(Some(result));
        }
        self.jobs.outcome(id)?;
        Ok(None)
    }

    async fn run_sync(&self, ctx: &JobContext) -> Result<Vec<CommunityTapSyncResult>, AppError> {
        let _lock = tokio::select! {
            biased;
            () = ctx.cancelled() => return Err(AppError::cancelled()),
            lock = self.sync_lock.lock() => lock,
        };
        check_cancelled(ctx)?;
        let settings = self.settings.clone();
        let settings = run_blocking(move || Ok(settings.load()?)).await?;
        check_cancelled(ctx)?;
        if settings.offline_mode {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "offline mode is enabled",
            ));
        }
        let runner = (self.runner_factory)(ctx.cancel_token().clone());
        let results = self
            .taps
            .sync_many_with_runner(&settings.community_taps, runner)
            .await?;
        check_cancelled(ctx)?;
        let metadata = self.metadata.clone();
        let ctx = ctx.clone();
        run_blocking(move || finalize_sync_results(&metadata, results, &ctx)).await
    }
}

/// Reaps the sync job and discards its result when the [`CommunityService::sync`]
/// waiter ends or is dropped. Dropping never cancels the job; it only abandons
/// the result, so the sync completes in the background.
struct SyncResultGuard {
    service: CommunityService,
    id: JobId,
    abandoned: Arc<AtomicBool>,
}

impl Drop for SyncResultGuard {
    fn drop(&mut self) {
        self.abandoned.store(true, Ordering::Release);
        if let Ok(mut results) = self.service.sync_results.lock() {
            results.remove(&self.id);
        }
        let jobs = self.service.jobs.clone();
        let id = self.id.clone();
        match jobs.outcome(&id) {
            Ok(Some(_)) => {
                let _ = jobs.reap(&id);
            }
            Ok(None) => {
                if let (Ok(mut events), Ok(runtime)) =
                    (jobs.subscribe(&id), tokio::runtime::Handle::try_current())
                {
                    drop(runtime.spawn(async move {
                        while let Some(event) = events.recv().await {
                            if matches!(event.kind, JobEventKind::Finished(_)) {
                                break;
                            }
                        }
                        let _ = jobs.reap(&id);
                    }));
                }
            }
            Err(_) => {}
        }
    }
}

fn result_slot_error() -> AppError {
    AppError::new(
        AppErrorKind::Internal,
        "community sync result is unavailable",
    )
}
