//! `RunExecutableService::start`: prepare off the executor, then one
//! supervised job keyed on the resolved prefix that runs the executable,
//! sweeps after a stop and removes a throwaway prefix before completing.

use std::path::{Path, PathBuf};

use crosshook_core::run_executable::{
    adhoc_prefix_root, prepare_run_executable, validate_run_executable_request,
    PreparedRunExecutable, RunExecutableError, RunExecutableRequest, RunExecutableResult,
    RunExecutableValidationError,
};

use super::types::{RunCompletion, RunExecutableResponse, RunExecutableStartError};
use super::{CleanupFn, RunExecutableService, RUN_EXECUTABLE_JOB_KIND};
use crate::jobs::{
    create_log_file_in, kill_processes_using_prefix, slugify_target, JobKey, JobSpec,
};
use crate::services::process::child::LogFile;
use crate::services::process::owned::{start_owned, FailureText, OwnedRun};
use crate::services::process::{prefix_job_key, JobOwner, ProcessRun};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Text of the successful launch result (unchanged wire text).
const MSG_LAUNCHED: &str = "Executable launched.";

/// Length of the per-run throwaway directory suffix (hex digits of a v4
/// UUID).
const RUN_SUFFIX_LEN: usize = 12;

/// The runner's failure texts (core's wording).
const RUNNER_TEXT: FailureText = FailureText {
    launch: |message| RunExecutableError::RunnerSpawnFailed { message }.message(),
    exited: |status| RunExecutableError::RunnerExitedWithFailure { status }.message(),
};

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

    /// After the child: a stop also kills the Wine processes outside the
    /// group (wineserver); then a throwaway prefix is removed.
    async fn finish(self, killed: bool) {
        if killed {
            let swept = self.path.clone();
            if let Err(error) =
                tokio::task::spawn_blocking(move || kill_processes_using_prefix(&swept)).await
            {
                tracing::warn!(%error, "run-executable stop sweep task failed");
            }
        }
        self.cleanup().await;
    }
}

impl RunExecutableService {
    /// Validates `request`, resolves and provisions its prefix (a unique
    /// throwaway prefix when `prefix_path` is empty), then runs the
    /// executable as a job holding the resolved prefix's key and owned by
    /// `owner` (for [`Self::cancel`] and [`Self::stop`]).
    ///
    /// Returns once the executable was spawned: the response carries the log
    /// path, the resolved prefix and the run id; the run streams the log,
    /// then finishes with the exit code or terminating signal. A throwaway
    /// prefix is removed before the completion is sent, whichever way the
    /// run ended.
    ///
    /// # Errors
    /// Nothing runs on any error, and a throwaway prefix created by this
    /// attempt is removed:
    /// - `Failed(Conflict)` ("another run is already active for this
    ///   window") while `owner`'s previous run is still active.
    /// - `Validation(variant)` for an invalid request.
    /// - `Failed(Conflict)` ("another operation is already running for this
    ///   prefix") when another job holds the prefix.
    /// - `Failed(Io)` when the executable cannot be spawned (core's launch
    ///   text), `Failed(Cancelled)` when `owner` cancelled or stopped before
    ///   it started, and `Failed(_)` when the prefix or log cannot be created
    ///   or no runtime is available.
    pub async fn start(
        &self,
        owner: JobOwner,
        request: RunExecutableRequest,
    ) -> Result<(RunExecutableResponse, ProcessRun<RunCompletion>), RunExecutableStartError> {
        let slot = self.active.claim(owner)?;
        let log_dir = self.log_dir.clone();
        let adhoc_root = self.adhoc_root.clone();
        let cleanup = self.cleanup.clone();
        let (key, prefix, run) =
            run_blocking(move || Ok(prepare(&request, adhoc_root, log_dir, cleanup)))
                .await
                .map_err(RunExecutableStartError::Failed)??;
        let log_path = run.log.path.display().to_string();
        let resolved_prefix_path = prefix.path.display().to_string();
        let spec = JobSpec::new(RUN_EXECUTABLE_JOB_KIND).with_key(key);
        let job_prefix = prefix.clone();
        let run = match start_owned(&self.jobs, spec, slot, run, move |killed| {
            job_prefix.finish(killed)
        })
        .await
        {
            Ok(run) => run,
            Err(error) => {
                // A refused start never ran the body; the unique throwaway
                // prefix of this attempt belongs to no other run.
                prefix.cleanup().await;
                return Err(error.into());
            }
        };
        let response = RunExecutableResponse {
            result: RunExecutableResult {
                succeeded: true,
                message: MSG_LAUNCHED.to_string(),
                helper_log_path: log_path,
                resolved_prefix_path,
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
) -> Result<(JobKey, RunPrefix, OwnedRun), RunExecutableStartError> {
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
            prefix,
            OwnedRun {
                command,
                log,
                text: RUNNER_TEXT,
            },
        )),
        Err(error) => {
            prefix.cleanup_blocking();
            Err(error)
        }
    }
}

/// The resolved prefix's exclusivity key and the run's log file.
///
/// The prefix was just provisioned, so resolving it only fails when it
/// vanished or became unreadable meanwhile: an explicit prefix then reports
/// [`RunExecutableValidationError::PrefixPathMissing`] on its field, a
/// throwaway prefix (no user field) an `Io` failure.
fn key_and_log(
    request: &RunExecutableRequest,
    prefix: &RunPrefix,
    log_dir: PathBuf,
) -> Result<(JobKey, LogFile), RunExecutableStartError> {
    let (_, key) = prefix_job_key(&prefix.path.to_string_lossy()).map_err(|error| {
        if prefix.throwaway_root.is_some() {
            RunExecutableStartError::Failed(AppError::new(AppErrorKind::Io, error.message()))
        } else {
            RunExecutableStartError::Validation(RunExecutableValidationError::PrefixPathMissing)
        }
    })?;
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
        RunExecutableError::RunnerSpawnFailed { .. }
        | RunExecutableError::RunnerWaitFailed { .. }
        | RunExecutableError::RunnerExitedWithFailure { .. } => AppErrorKind::Internal,
    };
    RunExecutableStartError::Failed(AppError::new(kind, error.message()))
}
