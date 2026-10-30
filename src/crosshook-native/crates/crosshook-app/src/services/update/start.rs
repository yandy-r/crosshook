//! `UpdateService::start`: prepare off the executor, then one supervised
//! updater job keyed on its prefix.

use std::path::PathBuf;

use crosshook_core::update::{
    build_update_command, validate_update_request, UpdateGameError, UpdateGameRequest,
    UpdateGameResult,
};

use super::types::{UpdateCompletion, UpdateGameResponse};
use super::{validation_error, UpdateService, UPDATE_GAME_JOB_KIND};
use crate::jobs::{create_log_file_in, slugify_target, JobKey, JobSpec};
use crate::services::process::child::LogFile;
use crate::services::process::owned::{start_owned, FailureText, OwnedRun};
use crate::services::process::{prefix_job_key, JobOwner, ProcessRun};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Text of the successful launch result (unchanged wire text).
const MSG_LAUNCHED: &str = "Update process launched.";

/// The updater's failure texts (core's wording).
const UPDATER_TEXT: FailureText = FailureText {
    launch: |message| UpdateGameError::UpdaterSpawnFailed { message }.message(),
    exited: |status| UpdateGameError::UpdaterExitedWithFailure { status }.message(),
};

impl UpdateService {
    /// Validates `request`, then runs the updater as a job holding the
    /// request's prefix key and owned by `owner` (for [`Self::cancel`]).
    ///
    /// Returns once the updater was spawned: the response carries the log
    /// path and the run id; the run streams the log, then finishes with the
    /// updater's exit code or terminating signal.
    ///
    /// # Errors
    /// Nothing runs on any error:
    /// - `Conflict` ("another run is already active for this window") while
    ///   `owner`'s previous update is still active.
    /// - `Validation { field }` for an invalid request.
    /// - `Conflict` ("another operation is already running for this prefix")
    ///   when another job holds the prefix.
    /// - `Io` when the log file cannot be created or the updater cannot be
    ///   spawned (core's launch text).
    /// - `Cancelled` when `owner` cancelled before the updater started.
    pub async fn start(
        &self,
        owner: JobOwner,
        request: UpdateGameRequest,
    ) -> Result<(UpdateGameResponse, ProcessRun<UpdateCompletion>), AppError> {
        let slot = self.active.claim(owner)?;
        let log_dir = self.log_dir.clone();
        let (key, run) = run_blocking(move || prepare(&request, log_dir)).await?;
        let log_path = run.log.path.display().to_string();
        let spec = JobSpec::new(UPDATE_GAME_JOB_KIND).with_key(key);
        let run = start_owned(&self.jobs, spec, slot, run, |_| async {}).await?;
        let response = UpdateGameResponse {
            result: UpdateGameResult {
                succeeded: true,
                message: MSG_LAUNCHED.to_string(),
                helper_log_path: log_path,
            },
            run_id: run.run_id().to_string(),
        };
        Ok((response, run))
    }
}

/// Validates, resolves the prefix key, builds the command and creates the
/// owner-only log file.
fn prepare(request: &UpdateGameRequest, log_dir: PathBuf) -> Result<(JobKey, OwnedRun), AppError> {
    validate_update_request(request).map_err(validation_error)?;
    let (_, key) = prefix_job_key(request.prefix_path.trim())?;
    let slug = slugify_target(&request.profile_name, "update");
    let (path, file) = create_log_file_in(log_dir, "update", &slug).map_err(|error| {
        AppError::new(
            AppErrorKind::Io,
            format!("failed to create the update log: {error}"),
        )
    })?;
    Ok((
        key,
        OwnedRun {
            command: build_update_command(request),
            log: LogFile { path, file },
            text: UPDATER_TEXT,
        },
    ))
}
