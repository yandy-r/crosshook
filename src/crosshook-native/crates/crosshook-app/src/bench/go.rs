//! The runner's go signal: a single `GO\n` line on stdin, read on a plain thread with
//! `poll(2)` and a deadline. `tokio::io::stdin` cannot be cancelled and would hold up runtime
//! shutdown, so it is never used; the thread dies with the process.
//!
//! Only a deferred replay (bench mode on, a parked plan) ever reads stdin.

use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use tokio::sync::oneshot;

const GO_LINE: &[u8] = b"GO\n";
/// Most bytes the reader accepts; anything beyond [`GO_LINE`] is already an error.
const MAX_GO_BYTES: usize = 8;

/// Wait until `fd` is readable. `Ok(false)`: the deadline passed first.
fn wait_readable(fd: BorrowedFd<'_>, deadline: Instant) -> Result<bool, String> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(false);
        }
        let ms = u16::try_from(left.as_millis().clamp(1, u128::from(u16::MAX))).unwrap_or(u16::MAX);
        let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
        match poll(&mut fds, PollTimeout::from(ms)) {
            Ok(0) | Err(Errno::EINTR) => {}
            Ok(_) => return Ok(true),
            Err(error) => return Err(format!("poll on stdin failed: {error}")),
        }
    }
}

/// Read the go line from `src`: exactly `GO\n`, within `timeout`. EOF, a timeout, a read error or
/// any other bytes are errors; the text never echoes what was read.
pub(super) fn read_go<R: Read + AsFd>(src: &mut R, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; MAX_GO_BYTES];
    let mut len = 0usize;
    loop {
        if !wait_readable(src.as_fd(), deadline)? {
            return Err("timed out waiting for the GO line on stdin".to_string());
        }
        let read = src
            .read(&mut buf[len..])
            .map_err(|e| format!("reading stdin failed: {e}"))?;
        if read == 0 {
            return Err("stdin closed before the GO line".to_string());
        }
        len += read;
        let seen = &buf[..len];
        let verdict_due = len >= GO_LINE.len();
        if seen == GO_LINE {
            return Ok(());
        }
        if verdict_due || !GO_LINE.starts_with(seen) {
            return Err("stdin carried something other than the GO line".to_string());
        }
    }
}

/// [`read_go`] on this process's stdin, through a private duplicate of fd 0 so std's stdin
/// buffer never holds bytes back.
pub(super) fn wait_for_go(timeout: Duration) -> Result<(), String> {
    let fd = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|e| format!("stdin unavailable: {e}"))?;
    read_go(&mut File::from(fd), timeout)
}

/// Run a blocking reader on its own thread and await its answer without blocking the runtime.
pub(super) async fn await_reader(
    reader: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("bench-go".to_string())
        .spawn(move || {
            // A closed receiver means the replay task is gone; nothing to tell.
            let _ = tx.send(reader());
        })
        .map_err(|e| format!("could not start the GO reader: {e}"))?;
    rx.await
        .unwrap_or_else(|_| Err("the GO reader stopped without an answer".to_string()))
}

/// Wait for the runner's `GO\n` on stdin for at most `timeout`.
pub(super) async fn await_go(timeout: Duration) -> Result<(), String> {
    await_reader(move || wait_for_go(timeout)).await
}

#[cfg(test)]
mod tests;
