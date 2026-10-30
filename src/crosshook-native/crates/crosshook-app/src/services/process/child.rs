//! Spawns and supervises one job child while its log streams to a sink.

use std::fmt;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::tee::spawn_pipe_tee_to_file;
use crate::jobs::{supervise_child_with_kill, ChildExit, JobContext, DEFAULT_KILL_GRACE};
use crate::log_tail::{follow_file, LogBatch, TailConfig};

/// How long the pipes may stay open after the child exited before the tee is
/// abandoned: a descendant that outlives the leader (or left its group) can
/// hold them forever.
pub(crate) const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// How the child's stdout and stderr reach the log file.
pub(crate) enum ChildOutput {
    /// Both streams are attached directly to the log file handle.
    LogFile,
    /// Both streams are piped and teed into the log file.
    Piped,
}

/// The run's log file: its path and the open handle returned by
/// `create_log_file_in` (never reopened by path for writing, D11).
pub(crate) struct LogFile {
    pub(crate) path: PathBuf,
    pub(crate) file: std::fs::File,
}

/// Why [`run_child`] produced no exit.
#[derive(Debug)]
pub(crate) enum ChildError {
    /// The job was cancelled, or the caller stopped waiting for `started`,
    /// before the child could start; `started` was not sent and no child is
    /// left running.
    Cancelled,
    /// The child could not be spawned (or its stdio could not be set up);
    /// `started` was not sent and nothing ran.
    Spawn(io::Error),
    /// Supervising the running child failed; `started` was sent.
    Supervise(io::Error),
}

impl fmt::Display for ChildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("the process job was cancelled before it started"),
            Self::Spawn(error) | Self::Supervise(error) => error.fmt(f),
        }
    }
}

/// Runs `command` to completion as the job's child while its output streams
/// to `log` and, tailed with [`follow_file`] (pinned to the log handle), to
/// `sink`.
///
/// The child gets a null stdin, its own process group and `kill_on_drop`.
/// With [`ChildOutput::LogFile`] stdout and stderr are clones of the log
/// handle; with [`ChildOutput::Piped`] the lossy pipe tee writes through the
/// log handle. `started` is sent only once the child was spawned, so a
/// spawn failure reaches the caller of
/// [`StartingRun::started`](super::run::StartingRun::started) as the job's
/// error instead of as a started run.
///
/// Supervision ([`supervise_child_with_kill`] with the job's cancel token,
/// `kill` and [`DEFAULT_KILL_GRACE`]), the tee and the tail run
/// concurrently. After the child exited — also after a cooperative cancel —
/// the tee gets at most [`DRAIN_GRACE`] to reach EOF, so the last lines a
/// cancelled child wrote still reach the log; only `kill` abandons it at
/// once. Then the tail runs its bounded final drain. This function returns
/// only after that final drain: every line was delivered to `sink` before
/// the caller can publish a completion.
///
/// `sink` must be non-blocking (it runs on the tail's task).
///
/// # Errors
/// [`ChildError::Cancelled`] when the job was cancelled before the spawn or
/// the caller dropped the `started` receiver (a just-spawned group is
/// killed), [`ChildError::Spawn`] when the stdio clones or the spawn failed,
/// and [`ChildError::Supervise`] when supervision failed. A failing tail is
/// logged and does not fail the call.
pub(crate) async fn run_child(
    ctx: &JobContext,
    mut command: Command,
    log: LogFile,
    output: ChildOutput,
    kill: &CancellationToken,
    started: oneshot::Sender<()>,
    sink: impl FnMut(LogBatch) + Send,
) -> Result<ChildExit, ChildError> {
    let LogFile { path, file } = log;
    // Pins the tail to the created log file (dev + inode): a file swapped in
    // at the path is never read.
    let pin = file.try_clone().map_err(ChildError::Spawn)?;
    command.stdin(Stdio::null());
    let tee_file = match output {
        ChildOutput::LogFile => {
            command
                .stdout(Stdio::from(file.try_clone().map_err(ChildError::Spawn)?))
                .stderr(Stdio::from(file.try_clone().map_err(ChildError::Spawn)?));
            drop(file);
            None
        }
        ChildOutput::Piped => {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
            Some(file)
        }
    };
    command.process_group(0).kill_on_drop(true);
    let cancel = ctx.cancel_token();
    if cancel.is_cancelled() {
        return Err(ChildError::Cancelled);
    }
    let mut child = command.spawn().map_err(ChildError::Spawn)?;
    // The command still owns the parent's copies of the child's stdio.
    drop(command);
    if started.send(()).is_err() {
        // Nobody holds the run: end the just-spawned group and run nothing.
        let now = CancellationToken::new();
        now.cancel();
        if let Err(error) = supervise_child_with_kill(child, cancel, &now, DEFAULT_KILL_GRACE).await
        {
            tracing::warn!(%error, "unclaimed process child could not be killed");
        }
        return Err(ChildError::Cancelled);
    }
    let tee = tee_file.map(|file| spawn_pipe_tee_to_file(&mut child, &path, file));

    let tail_cancel = CancellationToken::new();
    let tail = follow_file(
        path.clone(),
        &pin,
        TailConfig::default(),
        tail_cancel.clone(),
        sink,
    );
    let supervised = async {
        let exit = supervise_child_with_kill(child, cancel, kill, DEFAULT_KILL_GRACE).await;
        if let Some(tee) = tee {
            if !tee.finish_within(DRAIN_GRACE, kill).await {
                tracing::debug!("process log pipes stayed open after exit; tee abandoned");
            }
        }
        // Everything the tee will ever write is on disk: the tail's final
        // drain now reads up to the end.
        tail_cancel.cancel();
        exit
    };
    let (exit, tail_result) = tokio::join!(supervised, tail);
    if let Err(error) = tail_result {
        tracing::warn!(%error, "process log follow failed");
    }
    exit.map_err(ChildError::Supervise)
}

/// The exit code (natural exit) and signal number (killed) of `exit`.
pub(crate) fn exit_code_and_signal(exit: &ChildExit) -> (Option<i32>, Option<i32>) {
    let status = exit.status();
    (status.code(), status.signal())
}
