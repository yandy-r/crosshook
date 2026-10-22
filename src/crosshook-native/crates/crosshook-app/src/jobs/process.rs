//! Child-process supervision and process-related helpers moved from the Tauri
//! shell: log-file creation, target slugs, and the Wine-prefix kill sweep.

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crosshook_core::platform::host_std_command;
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
    /// Cancelled; the child or another group member outlived the grace period
    /// after `SIGTERM` and the group was `SIGKILL`ed. Carries the child's own
    /// exit status (which may stem from `SIGTERM` if only members lingered).
    Killed(ExitStatus),
}

impl ChildExit {
    /// The child's exit status, whatever the path.
    pub fn status(&self) -> ExitStatus {
        match *self {
            Self::Natural(status) | Self::Terminated(status) | Self::Killed(status) => status,
        }
    }

    /// Whether the exit was caused by cancellation.
    pub fn was_cancelled(&self) -> bool {
        !matches!(self, Self::Natural(_))
    }
}

/// Waits for `child`; on `cancel`, signals its whole process group:
/// `SIGTERM`, then `SIGKILL` if the child or any other group member is still
/// alive after `grace`.
///
/// A child that already exited when cancellation is observed is reported as
/// [`ChildExit::Natural`]; surviving group members are then left alone. The
/// child **must** have been spawned as the leader of its own process group
/// (`Command::process_group(0)`), so that its pid is the group id; this is
/// verified on entry, before waiting. Only the direct child is reaped;
/// grandchildren that leave the group (`setsid`/`setpgid`) are neither
/// tracked nor signalled.
///
/// Dropping the returned future before it resolves (task abort, runtime
/// shutdown) `SIGKILL`s the group, unless the child already exited naturally
/// or the final signal was already sent.
///
/// # Flatpak
/// Under Flatpak a host command spawned through `flatpak-spawn --host` makes
/// the local child the `flatpak-spawn` proxy, not the host process: these
/// signals reach only the proxy and the sandbox-side group. The host process
/// dies with the proxy only when it was spawned with `--watch-bus`, and the
/// only builder that adds it
/// (`crosshook_core::platform::host_command_with_env_and_directory_lifecycle_inner`)
/// is crate-private to `crosshook-core`; the public `host_command*` builders
/// do not. Until a lifecycle-bound builder is exported, supervising a host
/// command under Flatpak can leave the host process running after cancel.
///
/// # Errors
/// - [`io::ErrorKind::InvalidInput`] when the child is not a process-group
///   leader; nothing is waited on or signalled and the child is left running
///   (dropped).
/// - I/O errors from waiting, or a failed `getpgid`/`killpg` other than
///   `ESRCH` (group already gone).
pub async fn supervise_child(
    mut child: Child,
    cancel: &CancellationToken,
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
    tokio::select! {
        biased;
        status = child.wait() => {
            let status = status?;
            guard.disarm();
            return Ok(ChildExit::Natural(status));
        }
        () = cancel.cancelled() => {}
    }
    if let Some(status) = child.try_wait()? {
        guard.disarm();
        return Ok(ChildExit::Natural(status));
    }

    let deadline = Instant::now() + grace;
    signal_group(pgid, Signal::SIGTERM)?;
    if let Ok(status) = tokio::time::timeout_at(deadline, child.wait()).await {
        let status = status?;
        // The leader is gone; other members may still ignore `SIGTERM`.
        loop {
            if !group_alive(pgid)? {
                guard.disarm();
                return Ok(ChildExit::Terminated(status));
            }
            let now = Instant::now();
            if now >= deadline {
                signal_group(pgid, Signal::SIGKILL)?;
                guard.disarm();
                return Ok(ChildExit::Killed(status));
            }
            tokio::time::sleep((deadline - now).min(GROUP_POLL_INTERVAL)).await;
        }
    }
    if !signal_group(pgid, Signal::SIGKILL)? {
        // No group to signal; make sure the child itself cannot hang the wait.
        let _ = child.start_kill();
    }
    guard.disarm();
    child.wait().await.map(ChildExit::Killed)
}

/// `SIGKILL`s the supervised group when [`supervise_child`] is dropped (or
/// errors) before reaching a final result.
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

/// Creates an empty `/tmp/crosshook-logs/<prefix>-<slug>-<millis>.log` file
/// and returns its path.
///
/// # Errors
/// The I/O error message when the directory or file cannot be created.
pub fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let file_name = format!("{prefix}-{target_slug}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}

/// Lowercases ASCII alphanumerics, maps everything else to `-`, trims `-`;
/// returns `fallback` when nothing remains.
pub fn slugify_target(name: &str, fallback: &str) -> String {
    let slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Returns true if `entry` contains `prefix` at a path boundary: after a match
/// the next byte must be absent (end of entry), `/`, or `:` (PATH-style lists).
fn environ_entry_contains_prefix_path(entry: &[u8], prefix: &[u8]) -> bool {
    if prefix.is_empty() || entry.len() < prefix.len() {
        return false;
    }
    (0..=entry.len() - prefix.len()).any(|i| {
        if &entry[i..i + prefix.len()] != prefix {
            return false;
        }
        let after = i + prefix.len();
        after == entry.len() || matches!(entry[after], b'/' | b':')
    })
}

/// `SIGKILL`s every process whose `/proc/[pid]/environ` references the prefix
/// path.
///
/// Each `NUL`-separated entry is scanned for the prefix bytes. A match is
/// accepted only at a path boundary: the byte after the prefix must be the
/// end of the entry, `/` (e.g. `<prefix>/pfx`), or `:` (e.g. `PATH`-style
/// lists). This avoids false positives where a shorter prefix would match
/// inside a sibling path such as `<prefix>-other`.
///
/// Proton overrides `WINEPREFIX` for child processes (sets it to
/// `<prefix>/pfx` once it bootstraps the structure), but the parent wrapper,
/// intermediate scripts, and the game executable can each carry the prefix
/// path under different keys (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`,
/// `WINEDLLPATH`, …) with or without the `pfx` suffix.
///
/// Errors reading individual `/proc` entries are silently skipped — exited
/// processes, kernel threads, and processes owned by other users are
/// expected to fail and are not actionable.
pub fn kill_processes_using_prefix(prefix_path: &Path) {
    // Slugified ad-hoc prefixes are guaranteed ASCII by `slugify`. Assert
    // in debug builds so any future caller passing a non-UTF-8 path is
    // caught immediately rather than silently performing lossy substring
    // matching against `/proc/[pid]/environ` blobs.
    debug_assert!(
        prefix_path.to_str().is_some(),
        "kill_processes_using_prefix expects a UTF-8 path; got non-UTF-8 bytes which will be lossily converted",
    );
    let target_str = prefix_path.to_string_lossy().to_string();
    let target_bytes = target_str.as_bytes();
    if target_bytes.is_empty() {
        return;
    }

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(error) => {
            tracing::warn!(%error, "kill_processes_using_prefix: unable to read /proc");
            return;
        }
    };

    let mut killed = 0u32;
    for entry in proc_dir.flatten() {
        let name_os = entry.file_name();
        let name = match name_os.to_str() {
            Some(s) => s,
            None => continue,
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let environ_path = entry.path().join("environ");
        let environ_bytes = match std::fs::read(&environ_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let target_present = environ_bytes
            .split(|b| *b == b'\0')
            .filter(|entry| !entry.is_empty())
            .any(|entry| environ_entry_contains_prefix_path(entry, target_bytes));
        if !target_present {
            continue;
        }

        tracing::info!(
            pid = %name,
            prefix = %target_str,
            "kill_processes_using_prefix: SIGKILL"
        );
        let _ = host_std_command("kill").arg("-KILL").arg(name).status();
        killed += 1;
    }

    if killed > 0 {
        tracing::info!(
            killed,
            prefix = %target_str,
            "kill_processes_using_prefix: kill sweep complete"
        );
    }
}

#[cfg(test)]
mod tests;
