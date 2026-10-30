use std::collections::BTreeMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use crosshook_core::community::{GitInvocation, GitOutput, GitRunError, TapGitRunner};
use crosshook_core::platform;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio_util::sync::CancellationToken;

use crate::jobs::{supervise_child, DEFAULT_KILL_GRACE};
use crate::services::process::child::DRAIN_GRACE;

const OUTPUT_LIMIT: usize = 64 * 1024;

pub(super) struct SupervisedGitRunner {
    cancel: CancellationToken,
}

pub(super) fn default_runner_factory() -> super::RunnerFactory {
    Arc::new(|cancel| Arc::new(SupervisedGitRunner { cancel }))
}

impl TapGitRunner for SupervisedGitRunner {
    fn run<'a>(
        &'a self,
        invocation: GitInvocation,
    ) -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>> {
        Box::pin(async move {
            let child = spawn_command(&self.cancel, "git", &invocation)?;
            collect_output(child, &self.cancel).await
        })
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

fn spawn_command(
    cancel: &CancellationToken,
    program: &str,
    invocation: &GitInvocation,
) -> Result<Child, GitRunError> {
    if cancel.is_cancelled() {
        return Err(GitRunError::Cancelled);
    }
    let directory = invocation
        .directory
        .as_deref()
        .map(|path| {
            path.to_str().ok_or_else(|| {
                GitRunError::Failed(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "git working directory is not UTF-8",
                ))
            })
        })
        .transpose()?;
    platform::host_command_with_env_and_directory_lifecycle(
        program,
        &invocation.env,
        directory,
        &BTreeMap::new(),
    )
    .args(&invocation.args)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .process_group(0)
    .kill_on_drop(true)
    .spawn()
    .map_err(GitRunError::Failed)
}

async fn collect_output(
    child: Child,
    cancel: &CancellationToken,
) -> Result<GitOutput, GitRunError> {
    collect_output_with_grace(child, cancel, DRAIN_GRACE).await
}

/// Supervises `child` while draining both pipes, then bounds the drain.
///
/// The leader can exit while a descendant that left the process group (for
/// example an `ssh` ControlMaster) still holds the pipes, so EOF is not
/// guaranteed. Once supervision returns, the drains get `drain_grace` to reach
/// EOF and are abandoned on expiry or cancellation, keeping what was retained.
async fn collect_output_with_grace(
    mut child: Child,
    cancel: &CancellationToken,
    drain_grace: Duration,
) -> Result<GitOutput, GitRunError> {
    // These handles are always present: spawn_command configures both pipes.
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| GitRunError::Failed(io::Error::other("git stdout pipe is missing")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| GitRunError::Failed(io::Error::other("git stderr pipe is missing")))?;
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    // The drains own the pipes, so dropping them closes both read ends.
    let mut drains = Box::pin(async {
        tokio::join!(
            drain_into(stdout, &mut stdout_bytes),
            drain_into(stderr, &mut stderr_bytes),
        )
    });
    let mut supervise = Box::pin(supervise_child(child, cancel, DEFAULT_KILL_GRACE));
    let mut drained = None;
    // Drain concurrently with supervision so a full pipe cannot stall the child.
    // A pipe read failure must not drop the supervisor before it has killed and
    // reaped a cancelled child.
    let exit = loop {
        tokio::select! {
            exit = &mut supervise => break exit,
            result = &mut drains, if drained.is_none() => drained = Some(result),
        }
    };
    let mut abandoned_by_cancel = false;
    // A cancelled or failed supervision gets no drain wait: its result is
    // already decided and surviving pipe holders must not delay it.
    if drained.is_none() && exit.as_ref().is_ok_and(|exit| !exit.was_cancelled()) {
        tokio::select! {
            biased;
            result = &mut drains => drained = Some(result),
            () = cancel.cancelled() => abandoned_by_cancel = true,
            () = tokio::time::sleep(drain_grace) => {}
        }
    }
    drop(drains);
    drop(supervise);
    let exit = exit.map_err(GitRunError::Failed)?;
    if exit.was_cancelled() || abandoned_by_cancel {
        return Err(GitRunError::Cancelled);
    }
    if let Some((stdout_result, stderr_result)) = drained {
        stdout_result.map_err(GitRunError::Failed)?;
        stderr_result.map_err(GitRunError::Failed)?;
    }
    Ok(GitOutput {
        success: exit.status().success(),
        stdout: into_text(stdout_bytes),
        stderr: into_text(stderr_bytes),
    })
}

async fn drain_into(mut pipe: impl AsyncRead + Unpin, retained: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer = [0_u8; 8192];
    loop {
        let count = pipe.read(&mut buffer).await?;
        if count == 0 {
            return Ok(());
        }
        let keep = count.min(OUTPUT_LIMIT - retained.len());
        retained.extend_from_slice(&buffer[..keep]);
        // Continue reading after the retention cap so neither pipe can block
        // the child and prevent supervision from reaching its terminal state.
    }
}

fn into_text(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    }
}

/// Runs a controlled test child through the production spawn/drain/supervise path.
#[cfg(test)]
pub(super) async fn run_test_program(
    cancel: &CancellationToken,
    program: &str,
    invocation: GitInvocation,
    started: Option<tokio::sync::oneshot::Sender<u32>>,
) -> Result<GitOutput, GitRunError> {
    let child = spawn_command(cancel, program, &invocation)?;
    if let Some(started) = started {
        let pid = child
            .id()
            .ok_or_else(|| GitRunError::Failed(io::Error::other("test child has no pid")))?;
        let _ = started.send(pid);
    }
    collect_output(child, cancel).await
}

/// Like [`run_test_program`] with an explicit post-exit drain grace.
#[cfg(test)]
pub(super) async fn run_test_program_with_drain_grace(
    cancel: &CancellationToken,
    program: &str,
    invocation: GitInvocation,
    started: Option<tokio::sync::oneshot::Sender<u32>>,
    drain_grace: Duration,
) -> Result<GitOutput, GitRunError> {
    let child = spawn_command(cancel, program, &invocation)?;
    if let Some(started) = started {
        let pid = child
            .id()
            .ok_or_else(|| GitRunError::Failed(io::Error::other("test child has no pid")))?;
        let _ = started.send(pid);
    }
    collect_output_with_grace(child, cancel, drain_grace).await
}
