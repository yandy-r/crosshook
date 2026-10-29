//! `RunExecutableService::start`: prepare off the executor, then one
//! supervised job keyed on the resolved prefix that runs the executable,
//! sweeps after a stop and removes a throwaway prefix before completing.

use std::path::{Path, PathBuf};

use crosshook_core::run_executable::{
    adhoc_prefix_root, prepare_run_executable, validate_run_executable_request,
    PreparedRunExecutable, RunExecutableError, RunExecutableRequest, RunExecutableResult,
};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::types::{RunCompletion, RunExecutableResponse, RunExecutableStartError};
use super::{CleanupFn, RunExecutableService, RUN_EXECUTABLE_JOB_KIND};
use crate::jobs::{
    create_log_file_in, kill_processes_using_prefix, slugify_target, ChildExit, JobContext, JobKey,
    JobSpec, Outcome,
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
const MSG_LAUNCHED: &str = "Executable launched.";

/// Length of the per-run throwaway directory suffix (hex digits of a v4
/// UUID).
const RUN_SUFFIX_LEN: usize = 12;

/// Where a run's prefix lives and whether the job must delete it.
#[derive(Clone)]
struct RunPrefix {
    /// The resolved prefix (as handed to Proton).
    path: PathBuf,
    /// `Some(root)` for a throwaway prefix created under `root`; `None` for a
    /// prefix the user chose, which is never deleted.
    throwaway_root: Option<PathBuf>,
    /// The service's throwaway-prefix removal.
    cleanup: CleanupFn,
}

impl RunPrefix {
    /// Removes a throwaway prefix (blocking); a user prefix is left alone.
    fn cleanup_blocking(&self) {
        if let Some(root) = &self.throwaway_root {
            let outcome = (self.cleanup)(&self.path, root);
            tracing::debug!(?outcome, prefix = %self.path.display(), "throwaway prefix cleanup");
        }
    }

    /// [`Self::cleanup_blocking`] on the blocking pool.
    async fn cleanup(&self) {
        if self.throwaway_root.is_none() {
            return;
        }
        let prefix = self.clone();
        if let Err(error) = tokio::task::spawn_blocking(move || prefix.cleanup_blocking()).await {
            tracing::warn!(%error, "throwaway prefix cleanup task failed");
        }
    }
}

/// Everything the job body needs, prepared on the blocking pool.
struct RunJob {
    command: Command,
    prefix: RunPrefix,
    log: LogFile,
}

impl RunExecutableService {
    /// Validates `request`, resolves and provisions its prefix (a unique
    /// throwaway prefix when `prefix_path` is empty), then runs the
    /// executable as a job holding the resolved prefix's key and owned by
    /// `owner` (for [`Self::cancel`] and [`Self::stop`]).
    ///
    /// The returned response carries the log path, the resolved prefix and
    /// the run id; the run streams the log, then finishes with the exit code
    /// or terminating signal. A throwaway prefix is removed before the
    /// completion is sent, whichever way the run ended.
    ///
    /// # Errors
    /// - `Validation(variant)` for an invalid request; nothing runs.
    /// - `Failed(Conflict)` ("another operation is already running for this
    ///   prefix") when another job holds the prefix; nothing runs.
    /// - `Failed(_)` when the prefix or log cannot be created or no runtime
    ///   is available; nothing runs and a throwaway prefix created by this
    ///   attempt is removed.
    pub async fn start(
        &self,
        owner: JobOwner,
        request: RunExecutableRequest,
    ) -> Result<(RunExecutableResponse, ProcessRun<RunCompletion>), RunExecutableStartError> {
        let log_dir = self.log_dir.clone();
        let adhoc_root = self.adhoc_root.clone();
        let cleanup = self.cleanup.clone();
        let (key, job) = run_blocking(move || Ok(prepare(&request, adhoc_root, log_dir, cleanup)))
            .await
            .map_err(RunExecutableStartError::Failed)??;
        let log_path = job.log.path.clone();
        let prefix = job.prefix.clone();
        let active = self.active.clone();
        let spec = JobSpec::new(RUN_EXECUTABLE_JOB_KIND).with_key(key);
        let starting = match start_run(&self.jobs, spec, move |ctx, completion, started| {
            run_executable_job(ctx, job, owner, active, completion, started)
        }) {
            Ok(starting) => starting,
            Err(error) => {
                // Nothing ran: drop the empty log and the unique throwaway
                // prefix this attempt created (a user prefix is kept).
                remove_unused_log(&log_path);
                prefix.cleanup().await;
                return Err(conflict_as(error, MSG_PREFIX_BUSY).into());
            }
        };
        let run = match starting.started().await {
            Ok(run) => run,
            Err(error) => {
                // The job ended before its body ran (shutdown): its unique
                // throwaway prefix belongs to no other run.
                prefix.cleanup().await;
                return Err(error.into());
            }
        };
        let response = RunExecutableResponse {
            result: RunExecutableResult {
                succeeded: true,
                message: MSG_LAUNCHED.to_string(),
                helper_log_path: log_path.display().to_string(),
                resolved_prefix_path: prefix.path.display().to_string(),
            },
            run_id: run.run_id().to_string(),
        };
        Ok((response, run))
    }
}

/// Validates, resolves and provisions the prefix, takes its key, builds the
/// command and creates the owner-only log file. A throwaway prefix created
/// here is removed again when a later step fails.
fn prepare(
    request: &RunExecutableRequest,
    adhoc_root: Option<PathBuf>,
    log_dir: PathBuf,
    cleanup: CleanupFn,
) -> Result<(JobKey, RunJob), RunExecutableStartError> {
    validate_run_executable_request(request)?;
    let adhoc_root = match adhoc_root {
        Some(root) => root,
        None if request.prefix_path.trim().is_empty() => adhoc_prefix_root().map_err(core_error)?,
        // An explicit prefix never touches the throwaway namespace.
        None => PathBuf::new(),
    };
    let PreparedRunExecutable {
        command,
        prefix_path,
        is_throwaway,
    } = prepare_run_executable(request, &adhoc_root, &run_suffix()).map_err(core_error)?;
    let prefix = RunPrefix {
        path: prefix_path,
        throwaway_root: is_throwaway.then_some(adhoc_root),
        cleanup,
    };
    match key_and_log(request, &prefix, log_dir) {
        Ok((key, log)) => Ok((
            key,
            RunJob {
                command,
                prefix,
                log,
            },
        )),
        Err(error) => {
            prefix.cleanup_blocking();
            Err(error.into())
        }
    }
}

/// The resolved prefix's exclusivity key and the run's log file.
fn key_and_log(
    request: &RunExecutableRequest,
    prefix: &RunPrefix,
    log_dir: PathBuf,
) -> Result<(JobKey, LogFile), AppError> {
    let (_, key) = prefix_job_key(&prefix.path.to_string_lossy())?;
    let stem = Path::new(request.executable_path.trim())
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let slug = slugify_target(stem, "run-executable");
    let (path, file) = create_log_file_in(log_dir, "run-executable", &slug).map_err(|error| {
        AppError::new(
            AppErrorKind::Io,
            format!("failed to create the run-executable log: {error}"),
        )
    })?;
    Ok((key, LogFile { path, file }))
}

/// A fresh per-run suffix for a throwaway prefix directory.
fn run_suffix() -> String {
    let mut suffix = uuid::Uuid::new_v4().simple().to_string();
    suffix.truncate(RUN_SUFFIX_LEN);
    suffix
}

fn core_error(error: RunExecutableError) -> RunExecutableStartError {
    let kind = match &error {
        RunExecutableError::Validation(validation) => {
            return RunExecutableStartError::Validation(validation.clone());
        }
        RunExecutableError::PrefixCreationFailed { .. } => AppErrorKind::Io,
        RunExecutableError::HomeDirectoryUnavailable => AppErrorKind::Unavailable,
        RunExecutableError::RuntimeUnavailable
        | RunExecutableError::LogAttachmentFailed { .. }
        | RunExecutableError::RunnerSpawnFailed { .. }
        | RunExecutableError::RunnerWaitFailed { .. }
        | RunExecutableError::RunnerExitedWithFailure { .. } => AppErrorKind::Internal,
    };
    RunExecutableStartError::Failed(AppError::new(kind, error.message()))
}

fn remove_unused_log(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        tracing::debug!(%error, "unused run-executable log could not be removed");
    }
}

/// The job body: registers the owner slot, signals `started`, runs the
/// executable to completion, sweeps the prefix after a stop, removes a
/// throwaway prefix, and only then sends the completion — so the prefix key
/// is held until the prefix is gone.
async fn run_executable_job(
    ctx: JobContext,
    job: RunJob,
    owner: JobOwner,
    active: ActiveJobs,
    completion: oneshot::Sender<RunCompletion>,
    started: oneshot::Sender<()>,
) -> Outcome {
    let RunJob {
        command,
        prefix,
        log,
    } = job;
    let kill = CancellationToken::new();
    let _slot = active.register(
        owner,
        ActiveJob {
            id: ctx.id().clone(),
            kill: kill.clone(),
        },
    );
    if started.send(()).is_err() {
        // The caller is gone before the executable started: run nothing.
        prefix.cleanup().await;
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
    if kill.is_cancelled() {
        // Stop: also kill Wine processes outside the group (wineserver).
        let swept = prefix.path.clone();
        if let Err(error) =
            tokio::task::spawn_blocking(move || kill_processes_using_prefix(&swept)).await
        {
            tracing::warn!(%error, "run-executable stop sweep task failed");
        }
    }
    prefix.cleanup().await;
    match result {
        Ok(exit) => {
            let (code, signal) = exit_code_and_signal(&exit);
            let _ = completion.send(RunCompletion { code, signal });
            exit_outcome(&exit, code)
        }
        Err(error) => {
            let message = RunExecutableError::RunnerSpawnFailed {
                message: error.to_string(),
            }
            .message();
            ctx.log(vec![message.clone()]);
            let _ = completion.send(RunCompletion {
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
            RunExecutableError::RunnerExitedWithFailure { status: code }.message(),
        ))
    }
}
