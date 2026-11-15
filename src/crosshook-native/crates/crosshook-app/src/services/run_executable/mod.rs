//! Ad-hoc Windows executable runs: validation, one supervised Proton child
//! per run that holds its resolved prefix key (D5), live log streaming,
//! owner-checked cancel and stop (D6), and throwaway-prefix cleanup inside
//! the job before its completion.
//!
//! After a cancel or stop, and before any throwaway prefix is removed, the
//! job sweeps every process still using the prefix (Wine leaves the job's
//! process group, see [`crate::services::process::sweep`]). A throwaway
//! prefix is deleted only when that sweep left nothing behind.
//!
//! A run without an explicit prefix gets a unique throwaway prefix
//! `_run-adhoc/<slug>-<suffix>`, so two runs of executables with the same
//! stem never share (or delete) one prefix. The job keeps the prefix key
//! until that prefix has been removed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crosshook_core::run_executable::{validate_run_executable_request, RunExecutableRequest};

use crate::jobs::{sweep_prefix_until_clear, JobRegistry, PrefixSweep, LOG_DIR};
use crate::services::process::owner::ActiveJobs;
use crate::services::process::sweep::SWEEP_GRACE;
use crate::services::process::JobOwner;
use crate::services::run_blocking;

mod cleanup;
mod start;
mod types;

pub use types::{RunCompletion, RunExecutableResponse, RunExecutableStartError};

/// Registry kind for run-executable jobs.
pub const RUN_EXECUTABLE_JOB_KIND: &str = "run-executable";

/// Removes a throwaway prefix `(prefix, adhoc_root)` (blocking); the
/// production value is [`cleanup::cleanup_throwaway_prefix`].
type CleanupFn = Arc<dyn Fn(&Path, &Path) -> cleanup::CleanupOutcome + Send + Sync>;

/// Kills every process using a prefix until none is left (blocking); the
/// production value is [`sweep_prefix_until_clear`] with [`SWEEP_GRACE`].
type SweepFn = Arc<dyn Fn(&Path) -> PrefixSweep + Send + Sync>;

/// Runs Windows executables through Proton as supervised jobs.
#[derive(Clone)]
pub struct RunExecutableService {
    jobs: JobRegistry,
    active: ActiveJobs,
    /// Where run logs are created; [`LOG_DIR`] in production.
    log_dir: PathBuf,
    /// Throwaway namespace root; `None` resolves the platform root
    /// (`adhoc_prefix_root`) on each run that needs it.
    adhoc_root: Option<PathBuf>,
    /// Throwaway-prefix removal run by the job before its completion.
    cleanup: CleanupFn,
    /// The prefix sweep run after a cancel or stop and before a throwaway
    /// prefix is removed.
    sweep: SweepFn,
}

impl RunExecutableService {
    /// Builds the service over the shared job registry; logs go to
    /// [`LOG_DIR`] and throwaway prefixes under the platform `_run-adhoc`
    /// root.
    pub fn new(jobs: JobRegistry) -> Self {
        Self {
            jobs,
            active: ActiveJobs::default(),
            log_dir: PathBuf::from(LOG_DIR),
            adhoc_root: None,
            cleanup: Arc::new(cleanup::cleanup_throwaway_prefix),
            sweep: Arc::new(|prefix| sweep_prefix_until_clear(prefix, SWEEP_GRACE)),
        }
    }

    /// A service whose run logs land in `log_dir` and whose throwaway
    /// prefixes live under `adhoc_root` (test seam).
    #[cfg(test)]
    pub(crate) fn with_dirs(jobs: JobRegistry, log_dir: PathBuf, adhoc_root: PathBuf) -> Self {
        Self {
            adhoc_root: Some(adhoc_root),
            log_dir,
            ..Self::new(jobs)
        }
    }

    /// Replaces the throwaway-prefix removal (test seam, e.g. to gate it).
    #[cfg(test)]
    pub(crate) fn with_cleanup(
        mut self,
        cleanup: impl Fn(&Path, &Path) -> cleanup::CleanupOutcome + Send + Sync + 'static,
    ) -> Self {
        self.cleanup = Arc::new(cleanup);
        self
    }

    /// Replaces the prefix sweep (test seam, e.g. to report survivors).
    #[cfg(test)]
    pub(crate) fn with_sweep(
        mut self,
        sweep: impl Fn(&Path) -> PrefixSweep + Send + Sync + 'static,
    ) -> Self {
        self.sweep = Arc::new(sweep);
        self
    }

    /// The job registry the service's jobs run on.
    #[cfg(test)]
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }

    /// Validates the executable, Proton and optional prefix paths of
    /// `request`.
    ///
    /// # Errors
    /// `Validation(variant)` for an invalid request; `Failed` when the
    /// blocking validation task could not run.
    pub async fn validate(
        &self,
        request: RunExecutableRequest,
    ) -> Result<(), RunExecutableStartError> {
        run_blocking(move || Ok(validate_run_executable_request(&request)))
            .await
            .map_err(RunExecutableStartError::Failed)?
            .map_err(RunExecutableStartError::Validation)
    }

    /// Cancels `owner`'s active run: `SIGTERM` to its process group, then
    /// `SIGKILL` after the grace period, then (inside the job) a sweep of
    /// every process still using the resolved prefix; a run still being
    /// prepared ends without starting the executable. A throwaway prefix is
    /// removed before the run completes only when no process survived the
    /// sweep; otherwise it is kept and the run's log says so.
    ///
    /// Returns `false` when `owner` has no active run; no other job is
    /// touched.
    pub fn cancel(&self, owner: &JobOwner) -> bool {
        self.active.cancel(owner, &self.jobs)
    }

    /// Stops `owner`'s active run at once: `SIGKILL` to its process group
    /// without a grace period, then (inside the job) the same prefix sweep
    /// and throwaway-prefix rule as [`Self::cancel`].
    ///
    /// Returns `false` when `owner` has no active run; no other job is
    /// touched.
    pub fn stop(&self, owner: &JobOwner) -> bool {
        self.active.kill(owner)
    }
}

static_assertions::assert_impl_all!(RunExecutableService: Send, Sync);

#[cfg(test)]
mod tests;
