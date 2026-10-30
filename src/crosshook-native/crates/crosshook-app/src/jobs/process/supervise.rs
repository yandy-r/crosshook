//! Process-group supervision: natural exit, cooperative cancel
//! (`SIGTERM` → grace → `SIGKILL`) and immediate kill (`SIGKILL`).

use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::{getpgid, Pid};
use tokio::process::Child;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// Default grace period between `SIGTERM` and `SIGKILL`.
pub const DEFAULT_KILL_GRACE: Duration = Duration::from_secs(5);

/// How often the group is probed after its leader exited within the grace.
const GROUP_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// How a supervised child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildExit {
    /// Exited on its own before cancellation.
    Natural(ExitStatus),
    /// Cancelled; the whole group exited within the grace period after
    /// `SIGTERM`.
    Terminated(ExitStatus),
    /// Killed: either the kill token fired (immediate group `SIGKILL`), or the
    /// child or another group member outlived the grace period after
    /// `SIGTERM` and the group was `SIGKILL`ed. Carries the child's own exit
    /// status (which may stem from `SIGTERM` if only members lingered).
    Killed(ExitStatus),
}

impl ChildExit {
    /// The child's exit status, whatever the path.
    pub fn status(&self) -> ExitStatus {
        match *self {
            Self::Natural(status) | Self::Terminated(status) | Self::Killed(status) => status,
        }
    }

    /// Whether the exit was caused by cancellation or kill.
    pub fn was_cancelled(&self) -> bool {
        !matches!(self, Self::Natural(_))
    }
}

/// Waits for `child`; on `cancel`, signals its whole process group:
/// `SIGTERM`, then `SIGKILL` if the child or any other group member is still
/// alive after `grace`.
///
/// Equivalent to [`supervise_child_with_kill`] with a kill token that never
/// fires; see it for the full contract.
///
/// # Errors
/// As [`supervise_child_with_kill`].
pub async fn supervise_child(
    child: Child,
    cancel: &CancellationToken,
    grace: Duration,
) -> io::Result<ChildExit> {
    supervise_child_with_kill(child, cancel, &CancellationToken::new(), grace).await
}

/// Waits for `child`; on `cancel`, signals its whole process group:
/// `SIGTERM`, then `SIGKILL` if the child or any other group member is still
/// alive after `grace`. On `kill` (before or during that grace), the group is
/// `SIGKILL`ed immediately and the result is [`ChildExit::Killed`].
///
/// A child that already exited when cancellation or kill is observed is
/// reported as [`ChildExit::Natural`]; surviving group members are then left
/// alone. The child **must** have been spawned as the leader of its own
/// process group (`Command::process_group(0)`), so that its pid is the group
/// id; this is verified on entry, before waiting. Only the direct child is
/// reaped; grandchildren that leave the group (`setsid`/`setpgid`) are
/// neither tracked nor signalled.
///
/// Dropping the returned future before it resolves (task abort, runtime
/// shutdown) `SIGKILL`s the group, unless the child already exited naturally
/// or the final signal was already sent.
///
/// # Flatpak
/// Under Flatpak a host command spawned through `flatpak-spawn --host` makes
/// the local child the `flatpak-spawn` proxy, not the host process: these
/// signals reach only the proxy and the sandbox-side group. The host process
/// dies with the proxy only when it was spawned with `--watch-bus`, so host
/// commands supervised here must be built with
/// [`crosshook_core::platform::host_command_lifecycle`] or
/// [`crosshook_core::platform::host_command_with_env_and_directory_lifecycle`].
/// The other public `host_command*` builders omit `--watch-bus` and leave the
/// host process running after cancel.
///
/// # Errors
/// - [`io::ErrorKind::InvalidInput`] when the child is not a process-group
///   leader; nothing is waited on or signalled and the child is left running
///   (dropped).
/// - I/O errors from waiting, or a failed `getpgid`/`killpg` other than
///   `ESRCH` (group already gone).
pub async fn supervise_child_with_kill(
    mut child: Child,
    cancel: &CancellationToken,
    kill: &CancellationToken,
    grace: Duration,
) -> io::Result<ChildExit> {
    let Some(pid) = child.id() else {
        // Already reaped by the caller: report its cached status.
        return match child.try_wait()? {
            Some(status) => Ok(ChildExit::Natural(status)),
            None => Err(io::Error::other("child has no pid")),
        };
    };
    let pgid =
        Pid::from_raw(i32::try_from(pid).map_err(|_| io::Error::other("child pid out of range"))?);
    // The unreaped child (even a zombie) still answers `getpgid`.
    if getpgid(Some(pgid)).map_err(io::Error::from)? != pgid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "child is not a process-group leader",
        ));
    }
    let mut guard = GroupKillGuard { pgid, armed: true };

    // Prefer a natural exit that is already available over cancellation.
    // `try_wait` reaps an exited child synchronously; `wait()` alone may stay
    // pending until the SIGCHLD driver runs, even for a zombie.
    if let Some(status) = child.try_wait()? {
        guard.disarm();
        return Ok(ChildExit::Natural(status));
    }
    let killed = tokio::select! {
        biased;
        status = child.wait() => {
            let status = status?;
            guard.disarm();
            return Ok(ChildExit::Natural(status));
        }
        () = kill.cancelled() => true,
        () = cancel.cancelled() => false,
    };
    if let Some(status) = child.try_wait()? {
        guard.disarm();
        return Ok(ChildExit::Natural(status));
    }
    if killed {
        return kill_group_now(child, &mut guard).await;
    }

    let deadline = Instant::now() + grace;
    signal_group(pgid, Signal::SIGTERM)?;
    let status = tokio::select! {
        biased;
        status = tokio::time::timeout_at(deadline, child.wait()) => match status {
            Ok(status) => status?,
            Err(_elapsed) => return kill_group_now(child, &mut guard).await,
        },
        () = kill.cancelled() => return kill_group_now(child, &mut guard).await,
    };
    // The leader is gone; other members may still ignore `SIGTERM`.
    loop {
        if !group_alive(pgid)? {
            guard.disarm();
            return Ok(ChildExit::Terminated(status));
        }
        let now = Instant::now();
        if now >= deadline || kill.is_cancelled() {
            signal_group(pgid, Signal::SIGKILL)?;
            guard.disarm();
            return Ok(ChildExit::Killed(status));
        }
        tokio::select! {
            biased;
            () = kill.cancelled() => {}
            () = tokio::time::sleep((deadline - now).min(GROUP_POLL_INTERVAL)) => {}
        }
    }
}

/// `SIGKILL`s the group (or the child alone when the group is gone) and reaps
/// the child as [`ChildExit::Killed`].
async fn kill_group_now(mut child: Child, guard: &mut GroupKillGuard) -> io::Result<ChildExit> {
    if !signal_group(guard.pgid, Signal::SIGKILL)? {
        // No group to signal; make sure the child itself cannot hang the wait.
        let _ = child.start_kill();
    }
    guard.disarm();
    child.wait().await.map(ChildExit::Killed)
}

/// `SIGKILL`s the supervised group when supervision is dropped (or errors)
/// before reaching a final result.
struct GroupKillGuard {
    pgid: Pid,
    armed: bool,
}

impl GroupKillGuard {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for GroupKillGuard {
    fn drop(&mut self) {
        if self.armed {
            // ESRCH (group gone) and other failures are not actionable here.
            let _ = signal_group(self.pgid, Signal::SIGKILL);
        }
    }
}

/// Sends `signal` (`None` probes existence) to the group; `Ok(false)` when
/// the group no longer exists.
fn signal_group(pgid: Pid, signal: impl Into<Option<Signal>>) -> io::Result<bool> {
    match killpg(pgid, signal) {
        Ok(()) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(errno) => Err(io::Error::from(errno)),
    }
}

/// Whether any non-zombie process remains in the group.
///
/// `killpg(pgid, None)` is the fast path: it fails once the group is empty,
/// but also succeeds while only unreaped zombies (e.g. reparented members
/// awaiting their new parent) remain. In that case `/proc/*/stat` decides.
fn group_alive(pgid: Pid) -> io::Result<bool> {
    if !signal_group(pgid, None)? {
        return Ok(false);
    }
    let Ok(entries) = std::fs::read_dir("/proc") else {
        // Cannot tell zombies apart; trust `killpg`.
        return Ok(true);
    };
    let pgid = pgid.as_raw();
    Ok(entries.flatten().any(|entry| {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .filter(|name| name.bytes().all(|b| b.is_ascii_digit()))
        else {
            return false;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        // `pid (comm) state ppid pgrp ...`; `comm` may contain ')' or spaces.
        let Some((_, fields)) = stat.rsplit_once(')') else {
            return false;
        };
        let mut fields = fields.split_ascii_whitespace();
        let state = fields.next();
        let pgrp = fields.nth(1).and_then(|pgrp| pgrp.parse::<i32>().ok());
        pgrp == Some(pgid) && !matches!(state, Some("Z" | "X"))
    }))
}
