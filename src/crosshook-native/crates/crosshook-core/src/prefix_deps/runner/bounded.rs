//! Runs a short-lived prefix tool child (restore, `list-installed` check) in
//! its own process group with a hard timeout and a bounded output drain.
//!
//! On timeout (or when the future is dropped) the whole group is `SIGKILL`ed,
//! so Wine/regedit children of winetricks stop with it. After a natural exit,
//! surviving group members are left alone (winetricks waits for wineserver
//! itself), and the pipes get at most a short grace to reach EOF: a descendant
//! such as wineserver may hold them open indefinitely.

use std::io;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

/// How long the pipes may stay open after the child exited.
pub(super) const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Bytes retained per stream; the rest is read and discarded so a chatty
/// child never blocks on a full pipe.
const OUTPUT_LIMIT: usize = 256 * 1024;

/// Timeout and post-exit drain grace of one bounded run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    pub(super) timeout: Duration,
    pub(super) drain_grace: Duration,
}

/// Exit status and the retained output of a finished child.
#[derive(Debug)]
pub(super) struct BoundedOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

/// Why a bounded run produced no output.
#[derive(Debug)]
pub(super) enum BoundedError {
    /// The child could not be spawned.
    Spawn(io::Error),
    /// Waiting for the child failed; its group was killed.
    Wait(io::Error),
    /// The child outlived `timeout`; its group was killed.
    TimedOut,
}

/// Spawns `command` (null stdin, own process group, `kill_on_drop`; stdout and
/// stderr as configured by the caller) and waits up to `limits.timeout` while
/// draining its pipes.
///
/// # Errors
/// See [`BoundedError`].
pub(super) async fn run_bounded(
    mut command: Command,
    limits: Limits,
) -> Result<BoundedOutput, BoundedError> {
    command
        .stdin(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(BoundedError::Spawn)?;
    drop(command);
    let mut guard = GroupKillGuard {
        pgid: child
            .id()
            .and_then(|pid| i32::try_from(pid).ok())
            .map(Pid::from_raw),
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();

    let waited = {
        // The drains own the pipes: dropping them closes both read ends.
        let mut drains = Box::pin(async {
            tokio::join!(
                drain_into(stdout, &mut stdout_bytes),
                drain_into(stderr, &mut stderr_bytes),
            );
        });
        let mut drained = false;
        let mut wait = Box::pin(tokio::time::timeout(limits.timeout, child.wait()));
        // Drain while waiting so a full pipe cannot stall the child.
        let waited = loop {
            tokio::select! {
                waited = &mut wait => break waited,
                () = &mut drains, if !drained => drained = true,
            }
        };
        if matches!(waited, Ok(Ok(_)))
            && !drained
            && tokio::time::timeout(limits.drain_grace, &mut drains)
                .await
                .is_err()
        {
            tracing::debug!("prefix tool pipes stayed open after exit; drain abandoned");
        }
        waited
    };

    match waited {
        Ok(Ok(status)) => {
            // Reaped: the pgid may be reused from here on.
            guard.disarm();
            Ok(BoundedOutput {
                status,
                stdout: stdout_bytes,
                stderr: stderr_bytes,
            })
        }
        // The guard kills the unreaped group on return.
        Ok(Err(error)) => Err(BoundedError::Wait(error)),
        Err(_) => {
            // Unreaped leader: the pgid is still ours.
            guard.kill();
            let _ = child.wait().await;
            guard.disarm();
            Err(BoundedError::TimedOut)
        }
    }
}

async fn drain_into(pipe: Option<impl AsyncRead + Unpin>, retained: &mut Vec<u8>) {
    let Some(mut pipe) = pipe else {
        return;
    };
    let mut buffer = [0_u8; 8192];
    loop {
        match pipe.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(count) => {
                let keep = count.min(OUTPUT_LIMIT - retained.len());
                retained.extend_from_slice(&buffer[..keep]);
            }
        }
    }
}

/// `SIGKILL`s the child's group when the run is dropped or fails before the
/// leader was reaped.
struct GroupKillGuard {
    pgid: Option<Pid>,
}

impl GroupKillGuard {
    fn kill(&self) {
        if let Some(pgid) = self.pgid {
            match killpg(pgid, Signal::SIGKILL) {
                Ok(()) | Err(Errno::ESRCH) => {}
                Err(error) => tracing::warn!(%error, "failed to kill prefix tool process group"),
            }
        }
    }

    fn disarm(&mut self) {
        self.pgid = None;
    }
}

impl Drop for GroupKillGuard {
    fn drop(&mut self) {
        self.kill();
    }
}
