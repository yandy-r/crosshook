//! The installer run: prepare, take the prefix key, run the installer as a
//! supervised job, then build the reviewable result.

use std::path::PathBuf;

use crosshook_core::install::{
    finish_install, prepare_install, InstallGameError, InstallGameRequest, InstallGameResult,
    PreparedInstall,
};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::{install_error, InstallService, INSTALL_GAME_JOB_KIND};
use crate::jobs::{create_log_file_in, slugify_target, JobContext, JobKey, JobSpec, Outcome};
use crate::services::process::child::{run_child, ChildError, ChildOutput, LogFile};
use crate::services::process::run::start_run;
use crate::services::process::{
    conflict_as, prefix_job_key, remove_unused_log, ProcessRun, MSG_PREFIX_BUSY,
};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// What an install run completes with: the reviewable result, or the error
/// that ended it (failure text, or `Cancelled` on shutdown).
type InstallCompletion = Result<InstallGameResult, AppError>;

/// Everything the job body needs, prepared off the async executor.
struct PreparedRun {
    command: Command,
    prefix_path: PathBuf,
    key: JobKey,
    log: LogFile,
}

impl InstallService {
    /// Starts the installer for `request` as an `install-game` job.
    ///
    /// Validation and prefix provisioning (`prepare_install`) and log-file
    /// creation fail synchronously; then the job takes the prefix's
    /// exclusivity key and runs `<proton> run <installer>` with its output in
    /// the log file. The run is returned once the installer was spawned. Its
    /// completion is the reviewable result after a successful exit, the core
    /// failure text (`Io`) after a non-zero exit, or `Cancelled` when the job
    /// was cancelled (app shutdown), which also ends the installer's process
    /// group.
    ///
    /// # Errors
    /// `Validation { field }` for an invalid request, `Io` when the prefix or
    /// the log file cannot be created or the installer cannot be spawned,
    /// `Conflict` when another operation already holds the prefix, and
    /// `Cancelled` when the job was cancelled before the spawn. The empty log
    /// of an attempt that never started is removed.
    pub async fn start(
        &self,
        request: InstallGameRequest,
    ) -> Result<ProcessRun<Result<InstallGameResult, AppError>>, AppError> {
        let log_dir = self.log_dir.clone();
        let prepare_request = request.clone();
        let PreparedRun {
            command,
            prefix_path,
            key,
            log,
        } = run_blocking(move || prepare_run(&prepare_request, log_dir)).await?;
        let log_path = log.path.clone();

        let spec = JobSpec::new(INSTALL_GAME_JOB_KIND).with_key(key);
        let starting = start_run(&self.jobs, spec, move |ctx, completion, started| {
            run_installer(
                ctx,
                completion,
                started,
                InstallJob {
                    request,
                    command,
                    prefix_path,
                    log,
                },
            )
        });
        let run = match starting {
            Ok(starting) => starting.started().await,
            Err(error) => Err(conflict_as(error, MSG_PREFIX_BUSY)),
        };
        if run.is_err() {
            // Nothing ran: drop the empty log this attempt created.
            remove_unused_log(&log_path);
        }
        run
    }
}

/// Validates and provisions through core, derives the prefix key and creates
/// the run's log file.
fn prepare_run(request: &InstallGameRequest, log_dir: PathBuf) -> Result<PreparedRun, AppError> {
    let PreparedInstall {
        command,
        prefix_path,
    } = prepare_install(request).map_err(install_error)?;
    let (_, key) = prefix_job_key(&prefix_path.to_string_lossy())?;
    let (path, file) = create_log_file_in(
        log_dir,
        "install",
        &slugify_target(&request.profile_name, "install"),
    )
    .map_err(|message| AppError::new(AppErrorKind::Io, message))?;
    Ok(PreparedRun {
        command,
        prefix_path,
        key,
        log: LogFile { path, file },
    })
}

/// The job body's inputs.
struct InstallJob {
    request: InstallGameRequest,
    command: Command,
    prefix_path: PathBuf,
    log: LogFile,
}

/// The job body: runs the installer, then sends the completion.
async fn run_installer(
    ctx: JobContext,
    completion: oneshot::Sender<InstallCompletion>,
    started: oneshot::Sender<()>,
    job: InstallJob,
) -> Outcome {
    let InstallJob {
        request,
        command,
        prefix_path,
        log,
    } = job;
    let log_path = log.path.clone();
    let sink_ctx = ctx.clone();
    let exit = run_child(
        &ctx,
        command,
        log,
        ChildOutput::LogFile,
        &CancellationToken::new(),
        started,
        move |batch| sink_ctx.log_batch(batch),
    )
    .await;
    let result = match exit {
        // Nothing was spawned and `started` was not sent: the outcome
        // becomes the error of `start` itself, no completion is read.
        Err(ChildError::Cancelled) => return Outcome::Cancelled,
        Err(ChildError::Spawn(error)) => {
            return Outcome::Failed(failure(InstallGameError::InstallerSpawnFailed {
                message: error.to_string(),
            }));
        }
        Err(ChildError::Supervise(error)) => Err(failure(InstallGameError::InstallerSpawnFailed {
            message: error.to_string(),
        })),
        Ok(exit) if exit.was_cancelled() => Err(cancelled()),
        Ok(exit) if !exit.status().success() => {
            Err(failure(InstallGameError::InstallerExitedWithFailure {
                status: exit.status().code(),
            }))
        }
        Ok(_) => run_blocking(move || Ok(finish_install(&request, &prefix_path, &log_path))).await,
    };
    finish(completion, result)
}

/// Sends `result` as the completion and returns the matching job outcome.
fn finish(completion: oneshot::Sender<InstallCompletion>, result: InstallCompletion) -> Outcome {
    let outcome = match &result {
        Ok(_) => Outcome::Succeeded,
        Err(error) if error.kind() == AppErrorKind::Cancelled => Outcome::Cancelled,
        Err(error) => Outcome::Failed(error.clone()),
    };
    let _ = completion.send(result);
    outcome
}

/// The installer failure as an `Io` error carrying core's text.
fn failure(error: InstallGameError) -> AppError {
    AppError::new(AppErrorKind::Io, error.message())
}

/// The error for an installer ended by cancellation (app shutdown).
fn cancelled() -> AppError {
    AppError::new(AppErrorKind::Cancelled, "The installer was cancelled.")
}
