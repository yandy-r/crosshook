//! Reading and validating the gamescope host PID capture file.
//!
//! The capture file is written by the launch script inside the sandbox-visible
//! runtime directory, so its content is untrusted: it is read once, removed
//! immediately, strictly parsed, and the PID is only used after it matches a
//! `gamescope` process in the host process table.

use std::fs;
use std::io::{ErrorKind, Read};
use std::path::Path;

use crate::runtime_files::{open_private_read, RuntimeDirError};

use super::host::HostProcessEntry;

/// Longest capture file accepted; a `u32` PID has at most 10 digits plus a newline.
const MAX_CAPTURE_BYTES: u64 = 32;
const MAX_PID_DIGITS: usize = 10;
const GAMESCOPE_COMM: &str = "gamescope";

/// Result of one attempt to consume the capture file.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum CaptureRead {
    /// The file is missing, still empty, or briefly unreadable; keep polling.
    /// The file is left in place.
    Pending,
    /// The file was unsafe or held malformed content; it has been removed.
    Rejected,
    /// A syntactically valid PID greater than 1; the file has been removed.
    Pid(u32),
}

/// Reads, parses and unlinks the capture file on the blocking pool.
pub(super) async fn read_pid_capture_file_blocking(path: &Path) -> CaptureRead {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || read_pid_capture_file(&path))
        .await
        .unwrap_or(CaptureRead::Pending)
}

pub(super) fn read_pid_capture_file(path: &Path) -> CaptureRead {
    let file = match open_private_read(path) {
        Ok(file) => file,
        // Not written yet, or a transient failure such as `EMFILE`: retry.
        Err(RuntimeDirError::Io(_)) => return CaptureRead::Pending,
        Err(_) => {
            // Symlink, FIFO or foreign owner: never use it. A symlink is
            // unlinked itself, its target is untouched.
            remove_capture_file(path);
            return CaptureRead::Rejected;
        }
    };

    let mut content = Vec::new();
    if file
        .take(MAX_CAPTURE_BYTES + 1)
        .read_to_end(&mut content)
        .is_err()
    {
        return CaptureRead::Pending;
    }
    // `set -C; printf ... > file` creates the file before writing the PID, so
    // an empty file means the writer has not finished yet.
    if content.is_empty() {
        return CaptureRead::Pending;
    }
    remove_capture_file(path);

    match parse_capture_pid(&content) {
        Some(pid) => CaptureRead::Pid(pid),
        None => CaptureRead::Rejected,
    }
}

fn remove_capture_file(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != ErrorKind::NotFound {
            tracing::warn!(
                error = %error,
                "gamescope watchdog: could not remove the PID capture file"
            );
        }
    }
}

/// 1-10 ASCII digits after trimming, fitting a `u32`, greater than 1.
fn parse_capture_pid(content: &[u8]) -> Option<u32> {
    if content.len() as u64 > MAX_CAPTURE_BYTES {
        return None;
    }
    let text = std::str::from_utf8(content).ok()?.trim();
    if text.is_empty()
        || text.len() > MAX_PID_DIGITS
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    text.parse::<u32>().ok().filter(|pid| *pid > 1)
}

/// Who the captured PID must belong to: the current user, and not this process.
#[derive(Clone, Copy, Debug)]
pub(super) struct CaptureOwner {
    pub(super) uid: u32,
    pub(super) self_pid: u32,
}

impl CaptureOwner {
    pub(super) fn current() -> Self {
        Self {
            uid: nix::unistd::geteuid().as_raw(),
            self_pid: std::process::id(),
        }
    }
}

/// Whether `pid` is a gamescope process of `owner` in the given host snapshot.
///
/// A missing pid or a different `comm` (for example `bash` before `exec`) both
/// read as "not ready" so the caller can retry within its budget. An unknown
/// uid (the `/proc` fallback snapshot) never matches: the caller then uses the
/// host-ancestor search instead.
pub(super) fn pid_is_owned_gamescope(
    entries: &[HostProcessEntry],
    pid: u32,
    owner: CaptureOwner,
) -> bool {
    pid > 1
        && pid != owner.self_pid
        && entries.iter().any(|entry| {
            entry.pid == pid && entry.uid == Some(owner.uid) && entry.comm == GAMESCOPE_COMM
        })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    const OWNER: CaptureOwner = CaptureOwner {
        uid: 1000,
        self_pid: 50,
    };

    fn entry(pid: u32, uid: u32, comm: &str) -> HostProcessEntry {
        HostProcessEntry {
            pid,
            ppid: 1,
            uid: Some(uid),
            comm: comm.to_string(),
        }
    }

    #[test]
    fn predicate_accepts_only_gamescope_comm() {
        let entries = [entry(100, 1000, "bash"), entry(200, 1000, "gamescope")];
        assert!(pid_is_owned_gamescope(&entries, 200, OWNER));
        assert!(!pid_is_owned_gamescope(&entries, 100, OWNER));
        assert!(!pid_is_owned_gamescope(&entries, 300, OWNER));
    }

    #[test]
    fn predicate_rejects_pid_zero_one_and_self() {
        let entries = [
            entry(0, 1000, "gamescope"),
            entry(1, 1000, "gamescope"),
            entry(50, 1000, "gamescope"),
        ];
        assert!(!pid_is_owned_gamescope(&entries, 0, OWNER));
        assert!(!pid_is_owned_gamescope(&entries, 1, OWNER));
        assert!(!pid_is_owned_gamescope(&entries, 50, OWNER));
        assert!(!pid_is_owned_gamescope(&[], 200, OWNER));
    }

    #[test]
    fn predicate_rejects_another_users_gamescope() {
        let entries = [entry(200, 1001, "gamescope")];
        assert!(!pid_is_owned_gamescope(&entries, 200, OWNER));
    }

    #[test]
    fn empty_file_is_pending_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "").unwrap();
        assert_eq!(read_pid_capture_file(&path), CaptureRead::Pending);
        assert!(path.exists());

        fs::write(&path, "4242").unwrap();
        assert_eq!(read_pid_capture_file(&path), CaptureRead::Pid(4242));
    }

    #[test]
    fn valid_digits_return_pid_and_remove_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "4242\n").unwrap();
        assert_eq!(read_pid_capture_file(&path), CaptureRead::Pid(4242));
        assert!(!path.exists());
    }

    #[test]
    fn missing_file_is_pending() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read_pid_capture_file(&dir.path().join("pid")),
            CaptureRead::Pending
        );
    }

    #[test]
    fn malformed_content_is_rejected_and_removed() {
        let oversized = "1".repeat(33);
        let cases = [
            oversized.as_str(),
            "12ab",
            "-5",
            "1",
            "0",
            " \n",
            "99999999999",
            "4294967296",
        ];
        for content in cases {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("pid");
            fs::write(&path, content).unwrap();
            assert_eq!(
                read_pid_capture_file(&path),
                CaptureRead::Rejected,
                "content {content:?}"
            );
            assert!(!path.exists(), "content {content:?}");
        }
    }

    #[test]
    fn symlink_is_rejected_and_target_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        fs::write(&target, "4242").unwrap();
        let path = dir.path().join("pid");
        symlink(&target, &path).unwrap();

        assert_eq!(read_pid_capture_file(&path), CaptureRead::Rejected);
        assert!(fs::symlink_metadata(&path).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "4242");
    }

    #[test]
    fn fifo_is_rejected_without_blocking() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        nix::unistd::mkfifo(&path, nix::sys::stat::Mode::from_bits_truncate(0o600)).unwrap();

        let (tx, rx) = mpsc::channel();
        let thread_path = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(read_pid_capture_file(&thread_path));
        });
        let result = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("FIFO read must not block");
        assert_eq!(result, CaptureRead::Rejected);
        assert!(!path.exists());
    }
}
