//! Game updater runs: validation, one supervised Proton child per run that
//! holds its prefix key (D5), live log streaming, and owner-checked cancel
//! (D6).

use std::path::PathBuf;

use crosshook_core::update::{
    validate_update_request, UpdateGameRequest, UpdateGameValidationError,
};

use crate::jobs::JobRegistry;
use crate::services::process::owner::ActiveJobs;
use crate::services::process::JobOwner;
use crate::services::run_blocking;
use crate::AppError;

mod start;
mod types;

pub use types::{UpdateCompletion, UpdateGameResponse};

/// Registry kind for update-game jobs.
pub const UPDATE_GAME_JOB_KIND: &str = "update-game";

/// Runs game updaters through Proton as supervised jobs.
#[derive(Clone)]
pub struct UpdateService {
    jobs: JobRegistry,
    active: ActiveJobs,
    /// Where run logs are created; [`crate::jobs::LOG_DIR`] in production.
    log_dir: PathBuf,
}

impl UpdateService {
    /// Builds the service over the shared job registry; logs go to
    /// [`crate::jobs::LOG_DIR`].
    pub fn new(jobs: JobRegistry) -> Self {
        Self {
            jobs,
            active: ActiveJobs::default(),
            log_dir: PathBuf::from(crate::jobs::LOG_DIR),
        }
    }

    /// A service whose run logs land in `log_dir` (test seam).
    #[cfg(test)]
    pub(crate) fn with_log_dir(jobs: JobRegistry, log_dir: PathBuf) -> Self {
        Self {
            jobs,
            active: ActiveJobs::default(),
            log_dir,
        }
    }

    /// The job registry the service's jobs run on.
    #[cfg(test)]
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }

    /// Validates the updater, Proton and prefix paths of `request`.
    ///
    /// # Errors
    /// `Validation { field }` (`updater_path`, `proton_path` or
    /// `prefix_path`) carrying the core validation message.
    pub async fn validate(&self, request: UpdateGameRequest) -> Result<(), AppError> {
        run_blocking(move || validate_update_request(&request).map_err(validation_error)).await
    }

    /// Cancels `owner`'s active update: `SIGTERM` to its process group, then
    /// `SIGKILL` after the grace period; an update still being prepared ends
    /// without starting the updater.
    ///
    /// Returns `false` when `owner` has no active update; no other job is
    /// touched.
    pub fn cancel(&self, owner: &JobOwner) -> bool {
        self.active.cancel(owner, &self.jobs)
    }
}

/// Maps a core validation error to `Validation { field }` with its message.
fn validation_error(error: UpdateGameValidationError) -> AppError {
    use UpdateGameValidationError as E;
    let field = match error {
        E::UpdaterPathRequired
        | E::UpdaterPathNotAbsolute
        | E::UpdaterPathMissing
        | E::UpdaterPathNotFile
        | E::UpdaterPathNotWindowsExecutable => "updater_path",
        E::ProtonPathRequired
        | E::ProtonPathNotAbsolute
        | E::ProtonPathMissing
        | E::ProtonPathNotExecutable => "proton_path",
        E::PrefixPathRequired
        | E::PrefixPathNotAbsolute
        | E::PrefixPathMissing
        | E::PrefixPathNotDirectory => "prefix_path",
    };
    AppError::validation(field, error.message())
}

static_assertions::assert_impl_all!(UpdateService: Send, Sync);

#[cfg(test)]
mod tests;
