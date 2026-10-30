//! Explicit Windows-version repair and the keyed-job runner it shares with
//! startup recovery.

use std::future::Future;
use std::path::Path;

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::messages::msg_already_installing;
use super::{PrefixDepsService, PrefixVersionRepairStatus, PREFIX_REPAIR_JOB_KIND};
use crate::jobs::{JobContext, JobSpec, Outcome};
use crate::services::process::{conflict_as, prefix_job_key, prefix_job_key_for_path};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl PrefixDepsService {
    /// Restores and verifies the journaled Windows version of `prefix_path`
    /// (pending or failed row) in a `prefix-version-repair` job holding the
    /// prefix key, then returns the resulting status. A prefix without a row
    /// is already clean.
    ///
    /// # Errors
    /// `Validation { field: "prefix_path" }` for a missing prefix or a corrupt
    /// row, `Conflict` while another job holds the prefix, and the store's
    /// errors.
    pub async fn repair(&self, prefix_path: String) -> Result<PrefixVersionRepairStatus, AppError> {
        let (canonical, _) = run_blocking(move || prefix_job_key(&prefix_path)).await?;
        let journal = self.journal.clone();
        let prefix = canonical.clone();
        let result = self
            .run_keyed(
                PREFIX_REPAIR_JOB_KIND,
                &canonical,
                move |cancel| async move { journal.recover(&prefix, true, false, &cancel).await },
            )
            .await
            .map_err(|error| conflict_as(error, &msg_already_installing(&canonical)))?;
        tracing::info!(?result, "prefix Windows-version repair finished");
        // The repair supersedes any earlier automatic recovery result, and is
        // itself not an automatic recovery (`last_recovery`, D9).
        self.recoveries.clear(&canonical);
        let service = self.clone();
        run_blocking(move || {
            let mut status = service.journal.status(&canonical)?;
            status.last_recovery = service.recoveries.get(&canonical);
            Ok(status)
        })
        .await
    }

    /// Runs `work` as a `kind` job holding the key of the canonical prefix
    /// and returns its value once the job finished and was reaped. `work`
    /// gets the job's cancel token (cancelled only by app shutdown).
    ///
    /// `work` checks the token only before it starts a restore: a restore
    /// already running is bounded by its own timeout (I8), not the token.
    ///
    /// # Errors
    /// `Conflict` while another job holds the prefix, the registry's other
    /// start errors, and `Cancelled` when the job ended without a value
    /// (runtime shutdown).
    pub(super) async fn run_keyed<T, Fut>(
        &self,
        kind: &'static str,
        canonical: &str,
        work: impl FnOnce(CancellationToken) -> Fut + Send + 'static,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        let key = prefix_job_key_for_path(Path::new(canonical));
        let spec = JobSpec::new(kind).with_key(key);
        let (tx, rx) = oneshot::channel();
        let handle = self.jobs.start(spec, move |ctx: JobContext| async move {
            let _ = tx.send(work(ctx.cancel_token().clone()).await);
            Outcome::Succeeded
        })?;
        let id = handle.id().clone();
        let mut events = handle.into_events();
        while events.recv().await.is_some() {}
        if let Err(error) = self.jobs.reap(&id) {
            tracing::debug!(%error, "prefix job was already reaped");
        }
        rx.await
            .map_err(|_| AppError::new(AppErrorKind::Cancelled, "the prefix job was cancelled"))
    }
}
