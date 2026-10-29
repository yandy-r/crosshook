//! `UpdateService::start`: prepare off the executor, then one supervised
//! updater job keyed on its prefix.

use std::path::{Path, PathBuf};

use crosshook_core::update::{
    build_update_command, validate_update_request, UpdateGameError, UpdateGameRequest,
    UpdateGameResult,
};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::types::{UpdateCompletion, UpdateGameResponse};
use super::{validation_error, UpdateService, UPDATE_GAME_JOB_KIND};
use crate::jobs::{
    create_log_file_in, slugify_target, ChildExit, JobContext, JobKey, JobSpec, Outcome,
};
use crate::services::process::child::{exit_code_and_signal, run_child, ChildOutput, LogFile};
use crate::services::process::owner::{ActiveJob, ActiveJobs};
use crate::services::process::run::start_run;
use crate::services::process::{
    conflict_as, prefix_job_key, JobOwner, ProcessRun, MSG_PREFIX_BUSY,
};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Text of the successful launch result (unchanged wire text).
const MSG_LAUNCHED: &str = "Update process launched.";

/// Everything a run needs, prepared on the blocking pool.
struct PreparedUpdate {
    command: Command,
    key: JobKey,
    log: LogFile,
}

impl UpdateService {
    /// Validates `request`, then runs the updater as a job holding the
    /// request's prefix key and owned by `owner` (for [`Self::cancel`]).
    ///
    /// The returned response carries the log path and the run id; the run
    /// streams the log, then finishes with the updater's exit code or
    /// terminating signal.
    ///
    /// # Errors
    /// - `Validation { field }` for an invalid request; nothing runs.
    /// - `Conflict` ("another operation is already running for this prefix")
    ///   when another job holds the prefix; nothing runs.
    /// - `Io` when the log file cannot be created.
    pub async fn start(
        &self,
        owner: JobOwner,
        request: UpdateGameRequest,
    ) -> Result<(UpdateGameResponse, ProcessRun<UpdateCompletion>), AppError> {
        let log_dir = self.log_dir.clone();
        let PreparedUpdate { command, key, log } =
            run_blocking(move || prepare(&request, log_dir)).await?;
        let log_path = log.path.clone();
        let active = self.active.clone();
        let spec = JobSpec::new(UPDATE_GAME_JOB_KIND).with_key(key);
        let starting = start_run(&self.jobs, spec, move |ctx, completion, started| {
            run_updater(ctx, command, log, owner, active, completion, started)
        })
        .map_err(|error| {
            // Nothing ran: drop the empty log this attempt created.
            remove_unused_log(&log_path);
            conflict_as(error, MSG_PREFIX_BUSY)
        })?;
        let run = starting.started().await?;
        let response = UpdateGameResponse {
            result: UpdateGameResult {
                succeeded: true,
                message: MSG_LAUNCHED.to_string(),
                helper_log_path: log_path.display().to_string(),
            },
            run_id: run.run_id().to_string(),
        };
        Ok((response, run))
    }
}

/// Validates, resolves the prefix key, builds the command and creates the
/// owner-only log file.
fn prepare(request: &UpdateGameRequest, log_dir: PathBuf) -> Result<PreparedUpdate, AppError> {
    validate_update_request(request).map_err(validation_error)?;
    let (_, key) = prefix_job_key(request.prefix_path.trim())?;
    let command = build_update_command(request).map_err(command_error)?;
    let slug = slugify_target(&request.profile_name, "update");
    let (path, file) = create_log_file_in(log_dir, "update", &slug).map_err(|error| {
        AppError::new(
            AppErrorKind::Io,
            format!("failed to create the update log: {error}"),
        )
    })?;
    Ok(PreparedUpdate {
        command,
        key,
        log: LogFile { path, file },
    })
}

fn command_error(error: UpdateGameError) -> AppError {
    match error {
        UpdateGameError::Validation(validation) => validation_error(validation),
        other => AppError::new(AppErrorKind::Internal, other.message()),
    }
}

fn remove_unused_log(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        tracing::debug!(%error, "unused update log could not be removed");
    }
}

/// The job body: registers the owner slot, signals `started`, runs the
/// updater to completion and sends its completion after the final log drain.
async fn run_updater(
    ctx: JobContext,
    command: Command,
    log: LogFile,
    owner: JobOwner,
    active: ActiveJobs,
    completion: oneshot::Sender<UpdateCompletion>,
    started: oneshot::Sender<()>,
) -> Outcome {
    // Update has no stop: the kill token is never fired.
    let kill = CancellationToken::new();
    let _slot = active.register(
        owner,
        ActiveJob {
            id: ctx.id().clone(),
            kill: kill.clone(),
        },
    );
    if started.send(()).is_err() {
        // The caller is gone before the updater started: run nothing.
        return Outcome::Cancelled;
    }
    let sink_ctx = ctx.clone();
    let result = run_child(
        &ctx,
        command,
        log,
        ChildOutput::LogFile,
        &kill,
        move |batch| {
            sink_ctx.log_batch(batch);
        },
    )
    .await;
    match result {
        Ok(exit) => {
            let (code, signal) = exit_code_and_signal(&exit);
            let _ = completion.send(UpdateCompletion { code, signal });
            exit_outcome(&exit, code)
        }
        Err(error) => {
            let message = UpdateGameError::UpdaterSpawnFailed {
                message: error.to_string(),
            }
            .message();
            ctx.log(vec![message.clone()]);
            let _ = completion.send(UpdateCompletion {
                code: None,
                signal: None,
            });
            Outcome::Failed(AppError::new(AppErrorKind::Io, message))
        }
    }
}

fn exit_outcome(exit: &ChildExit, code: Option<i32>) -> Outcome {
    if exit.was_cancelled() {
        Outcome::Cancelled
    } else if exit.status().success() {
        Outcome::Succeeded
    } else {
        Outcome::Failed(AppError::new(
            AppErrorKind::Internal,
            UpdateGameError::UpdaterExitedWithFailure { status: code }.message(),
        ))
    }
}
