//! An owner-bound run whose completion is the child's exit code or signal:
//! the shared start, job body and finish of update and run-executable.

use std::future::Future;

use tokio::process::Command;
use tokio::sync::oneshot;

use super::child::{exit_code_and_signal, run_child, ChildError, ChildOutput, LogFile};
use super::owner::OwnerSlot;
use super::run::start_run;
use super::{conflict_as, remove_unused_log, ProcessRun, MSG_PREFIX_BUSY};
use crate::jobs::{ChildExit, JobContext, JobRegistry, JobSpec, Outcome};
use crate::{AppError, AppErrorKind};

/// How an owned run ended.
///
/// `code` is set for a natural exit and `signal` when the process was
/// terminated by a signal (cancel: `SIGTERM`, stop: `SIGKILL`). Both are
/// `None` when supervising the running process failed; the error text is
/// then in the run's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessCompletion {
    /// Exit code of a natural exit.
    pub code: Option<i32>,
    /// Signal number that terminated the process.
    pub signal: Option<i32>,
}

/// The service's failure texts (core's wording).
pub(crate) struct FailureText {
    /// The text of a spawn or supervision failure, given the I/O error text.
    pub(crate) launch: fn(String) -> String,
    /// The text of an unsuccessful exit, given the exit code.
    pub(crate) exited: fn(Option<i32>) -> String,
}

/// What an owned run executes.
pub(crate) struct OwnedRun {
    pub(crate) command: Command,
    pub(crate) log: LogFile,
    pub(crate) text: FailureText,
}

/// Starts `run` as a job owned by `slot` and returns once its child was
/// spawned.
///
/// The body binds `slot` to the job, runs the child with the slot's kill
/// token ([`run_child`], output straight into the log file), then awaits
/// `after_exit(killed)` still holding the job's key, and only then releases
/// the slot and sends the completion. `killed` is `true` when the slot's
/// kill token fired while a child ran. `after_exit` also runs (with `false`)
/// when no child was started.
///
/// # Errors
/// Nothing ran and the run's log was removed:
/// - `Conflict` ([`MSG_PREFIX_BUSY`]) when `spec`'s key is held,
/// - `Io` with `text.launch` when the child could not be spawned,
/// - `Cancelled` when the run was cancelled before its child started,
/// - the registry's other start errors.
pub(crate) async fn start_owned<F, Fut>(
    jobs: &JobRegistry,
    spec: JobSpec,
    slot: OwnerSlot,
    run: OwnedRun,
    after_exit: F,
) -> Result<ProcessRun<ProcessCompletion>, AppError>
where
    F: FnOnce(bool) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let log_path = run.log.path.clone();
    let started = match start_run(jobs, spec, move |ctx, completion, started| {
        owned_body(ctx, slot, run, completion, started, after_exit)
    }) {
        Ok(starting) => starting.started().await,
        Err(error) => Err(conflict_as(error, MSG_PREFIX_BUSY)),
    };
    if started.is_err() {
        remove_unused_log(&log_path);
    }
    started
}

async fn owned_body<F, Fut>(
    ctx: JobContext,
    slot: OwnerSlot,
    run: OwnedRun,
    completion: oneshot::Sender<ProcessCompletion>,
    started: oneshot::Sender<()>,
    after_exit: F,
) -> Outcome
where
    F: FnOnce(bool) -> Fut,
    Fut: Future<Output = ()>,
{
    let OwnedRun { command, log, text } = run;
    let kill = slot.kill_token().clone();
    if slot.bind(ctx.id()) || kill.is_cancelled() {
        // Cancelled or stopped while the run was being prepared: spawn
        // nothing (`started` is dropped unsent).
        after_exit(false).await;
        return Outcome::Cancelled;
    }
    let sink_ctx = ctx.clone();
    let result = run_child(
        &ctx,
        command,
        log,
        ChildOutput::LogFile,
        &kill,
        started,
        move |batch| sink_ctx.log_batch(batch),
    )
    .await;
    let ran = matches!(result, Ok(_) | Err(ChildError::Supervise(_)));
    after_exit(ran && kill.is_cancelled()).await;
    drop(slot);
    match result {
        Ok(exit) => {
            let (code, signal) = exit_code_and_signal(&exit);
            let _ = completion.send(ProcessCompletion { code, signal });
            exit_outcome(&exit, (text.exited)(code))
        }
        Err(ChildError::Cancelled) => Outcome::Cancelled,
        Err(ChildError::Spawn(error)) => Outcome::Failed(AppError::new(
            AppErrorKind::Io,
            (text.launch)(error.to_string()),
        )),
        Err(ChildError::Supervise(error)) => {
            let message = (text.launch)(error.to_string());
            ctx.log(vec![message.clone()]);
            let _ = completion.send(ProcessCompletion {
                code: None,
                signal: None,
            });
            Outcome::Failed(AppError::new(AppErrorKind::Io, message))
        }
    }
}

fn exit_outcome(exit: &ChildExit, failure: String) -> Outcome {
    if exit.was_cancelled() {
        Outcome::Cancelled
    } else if exit.status().success() {
        Outcome::Succeeded
    } else {
        Outcome::Failed(AppError::new(AppErrorKind::Internal, failure))
    }
}
