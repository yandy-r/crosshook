//! Spawns and supervises one job child while its log streams to a sink.

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::tee::spawn_pipe_tee_to_file;
use crate::jobs::{supervise_child_with_kill, ChildExit, JobContext, DEFAULT_KILL_GRACE};
use crate::log_tail::{follow, LogBatch, TailConfig};

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

/// Runs `command` to completion as the job's child while its output streams
/// to `log` and, tailed with [`follow`], to `sink`.
///
/// The child gets a null stdin, its own process group and `kill_on_drop`.
/// With [`ChildOutput::LogFile`] stdout and stderr are clones of the log
/// handle; with [`ChildOutput::Piped`] the lossy pipe tee writes through the
/// log handle. Supervision ([`supervise_child_with_kill`] with the job's
/// cancel token, `kill` and [`DEFAULT_KILL_GRACE`]), the tee and the tail run
/// concurrently. After the child exited, the tee gets at most
/// [`DRAIN_GRACE`] to reach EOF (abandoned at once when the job is
/// cancelled), then the tail runs its bounded final drain. This function
/// returns only after that final drain: every line was delivered to `sink`
/// before the caller can publish a completion.
///
/// `sink` must be non-blocking (it runs on the tail's task).
///
/// # Errors
/// Creating the log handle clones or spawning fails (nothing runs then), or
/// supervision fails. A failing tail is logged and does not fail the call.
pub(crate) async fn run_child(
    ctx: &JobContext,
    mut command: Command,
    log: LogFile,
    output: ChildOutput,
    kill: &CancellationToken,
    sink: impl FnMut(LogBatch) + Send,
) -> io::Result<ChildExit> {
    let LogFile { path, file } = log;
    command.stdin(Stdio::null());
    let tee_file = match output {
        ChildOutput::LogFile => {
            command
                .stdout(Stdio::from(file.try_clone()?))
                .stderr(Stdio::from(file.try_clone()?));
            drop(file);
            None
        }
        ChildOutput::Piped => {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
            Some(file)
        }
    };
    command.process_group(0).kill_on_drop(true);
    let mut child = command.spawn()?;
    // The command still owns the parent's copies of the child's stdio.
    drop(command);
    let tee = tee_file.map(|file| spawn_pipe_tee_to_file(&mut child, &path, file));

    let cancel = ctx.cancel_token();
    let tail_cancel = CancellationToken::new();
    let tail = follow(
        path.clone(),
        TailConfig::default(),
        tail_cancel.clone(),
        sink,
    );
    let supervised = async {
        let exit = supervise_child_with_kill(child, cancel, kill, DEFAULT_KILL_GRACE).await;
        if let Some(tee) = tee {
            if !tee.finish_within(DRAIN_GRACE, cancel).await {
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
    exit
}

/// The exit code (natural exit) and signal number (killed) of `exit`.
pub(crate) fn exit_code_and_signal(exit: &ChildExit) -> (Option<i32>, Option<i32>) {
    let status = exit.status();
    (status.code(), status.signal())
}
