//! Async driver of a [`LogTail`]: adaptive polling, per-window coalescing
//! and the bounded final drain.

use std::fs::File;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::{LogBatch, LogTail};

#[cfg(test)]
mod tests;

/// Polling cadence for [`follow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TailConfig {
    /// Poll interval while data flows; at most one batch is emitted per
    /// window. Clamped to `16..=33` ms.
    pub coalesce_window: Duration,
    /// Upper bound for the idle poll interval, which doubles on every empty
    /// poll. Never below the effective coalescing window.
    pub idle_backoff_max: Duration,
}

impl TailConfig {
    /// Smallest allowed coalescing window.
    pub const MIN_WINDOW: Duration = Duration::from_millis(16);
    /// Largest (and default) coalescing window.
    pub const MAX_WINDOW: Duration = Duration::from_millis(33);
    /// Default idle backoff ceiling.
    pub const DEFAULT_IDLE_BACKOFF_MAX: Duration = Duration::from_millis(500);

    fn effective(self) -> (Duration, Duration) {
        let window = self
            .coalesce_window
            .clamp(Self::MIN_WINDOW, Self::MAX_WINDOW);
        (window, self.idle_backoff_max.max(window))
    }
}

impl Default for TailConfig {
    fn default() -> Self {
        Self {
            coalesce_window: Self::MAX_WINDOW,
            idle_backoff_max: Self::DEFAULT_IDLE_BACKOFF_MAX,
        }
    }
}

/// Maximum polls [`follow`] runs after cancellation to drain a backlog, so
/// cancellation stays prompt even when a writer outpaces the reader. Each
/// poll reads at most [`READ_CHUNK`](super::READ_CHUNK) bytes and [`MAX_LINES_PER_POLL`](super::MAX_LINES_PER_POLL) lines,
/// so the drain reaches 16 MiB of long lines (64 × [`READ_CHUNK`](super::READ_CHUNK)) or about
/// 262 000 short ones. When this cap cuts the drain short while the tail
/// still reports a backlog, a [`LogBatch::Truncated`] notice estimates the
/// unread bytes instead of losing them silently.
pub const FINAL_DRAIN_CHUNKS: u32 = 64;

/// Consecutive poll errors [`drive`] tolerates before giving up. Between
/// errors the poll is retried with a doubling delay starting at the
/// coalescing window and capped at the idle backoff ceiling, so a transient
/// I/O error does not end streaming for the rest of the session.
const MAX_CONSECUTIVE_POLL_ERRORS: u32 = 8;

/// What a poll in [`drive`] must do with the buffered partial line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollMode {
    /// Normal poll while following; keep the partial line buffered.
    Live,
    /// Post-cancel drain poll; flush the partial line only if nothing more
    /// is pending.
    Drain,
    /// Last allowed post-cancel poll; always flush the partial line.
    Last,
}

/// One finished poll: the state handed back, the lines read, whether more is
/// already pending, and the unread-byte estimate when the final drain was cut
/// short.
type PollDone = (LogTail, Vec<String>, bool, Option<u64>);

/// Tails `path` until `cancel` fires, delivering coalesced lines to `sink`.
///
/// Polls once immediately, then every coalescing window while data flows;
/// while the tail reports a backlog the next poll runs without delay. Each
/// empty poll doubles the interval up to `idle_backoff_max`, and new data
/// resets it. All lines read by one poll (at most [`READ_CHUNK`](super::READ_CHUNK) bytes and
/// [`MAX_LINES_PER_POLL`](super::MAX_LINES_PER_POLL) lines) form one [`LogBatch::Lines`]; a larger
/// backlog is delivered over the following polls. Empty batches are never
/// emitted. On cancellation the remaining bytes are read without waiting, up
/// to [`FINAL_DRAIN_CHUNKS`] polls; if that cap cuts the drain short, a
/// [`LogBatch::Truncated`] notice estimates the unread bytes (never emitted
/// for 0). Then the unterminated trailing line (if any) is flushed and the
/// function returns. A live-phase poll I/O error is logged and retried with
/// backoff, up to a bounded streak. File reads run on Tokio's blocking pool,
/// so `sink` should be non-blocking.
///
/// # Errors
/// Returns the I/O error after a bounded streak of consecutive poll
/// failures, or a drain-phase poll error. Returns a terminal error at once
/// (no retry, since the tail state is lost) if the blocking read task
/// panicked or was cancelled.
pub async fn follow(
    path: impl Into<PathBuf>,
    config: TailConfig,
    cancel: CancellationToken,
    sink: impl FnMut(LogBatch),
) -> io::Result<()> {
    follow_tail(LogTail::new(path), config, cancel, sink).await
}

/// [`follow`] pinned to the file `created` refers to (the handle the log
/// was created with, see [`LogTail::pinned`]): if `path` names any other
/// file — swapped for a symlink, a FIFO or another file — nothing from it is
/// read and the log ends.
///
/// # Errors
/// The `fstat` error of `created`, and every error of [`follow`].
pub async fn follow_file(
    path: impl Into<PathBuf>,
    created: &File,
    config: TailConfig,
    cancel: CancellationToken,
    sink: impl FnMut(LogBatch),
) -> io::Result<()> {
    let tail = LogTail::pinned(path, created)?;
    follow_tail(tail, config, cancel, sink).await
}

async fn follow_tail(
    tail: LogTail,
    config: TailConfig,
    cancel: CancellationToken,
    sink: impl FnMut(LogBatch),
) -> io::Result<()> {
    let poll = |tail: LogTail, mode: PollMode| blocking_poll(tail, mode, poll_once);
    drive(tail, poll, config, cancel, sink).await
}

/// Synchronous body of one [`follow`] poll.
fn poll_once(tail: &mut LogTail, mode: PollMode) -> io::Result<(Vec<String>, bool, Option<u64>)> {
    let mut lines = tail.poll()?;
    let more = tail.has_backlog();
    if mode == PollMode::Last || (mode == PollMode::Drain && !more) {
        lines.extend(tail.flush_partial());
    }
    // The bounded drain was cut short with bytes still pending; estimate
    // them instead of losing them silently.
    let truncated = (mode == PollMode::Last && more).then(|| tail.unread_bytes());
    Ok((lines, more, truncated))
}

/// Runs `work` on the blocking pool. A task that panicked or was cancelled
/// has taken the tail with it, so that is reported as a terminal error (`None`
/// state) instead of unwinding into the caller.
async fn blocking_poll(
    mut tail: LogTail,
    mode: PollMode,
    work: fn(&mut LogTail, PollMode) -> io::Result<(Vec<String>, bool, Option<u64>)>,
) -> Result<PollDone, (Option<LogTail>, io::Error)> {
    let joined = tokio::task::spawn_blocking(move || {
        let result = work(&mut tail, mode);
        (tail, result)
    })
    .await;
    match joined {
        Ok((tail, Ok((lines, more, truncated)))) => Ok((tail, lines, more, truncated)),
        Ok((tail, Err(error))) => Err((Some(tail), error)),
        Err(join_error) => Err((
            None,
            io::Error::other(format!("log tail blocking read task failed: {join_error}")),
        )),
    }
}

/// Time-driven core of [`follow`], generic over the poll so the cadence can
/// be tested deterministically. The poll takes the state by value and hands
/// it back with the lines read, whether more bytes are already pending, and
/// an estimate of unread bytes when the bounded final drain is cut short.
/// While it reports more pending, the next poll runs without delay. After
/// cancellation it is called with [`PollMode::Drain`] without delay while it
/// reports more pending, at most [`FINAL_DRAIN_CHUNKS`] times in total, the
/// last of which uses [`PollMode::Last`]. A live-phase poll error that hands
/// the state back (`Some`) is retried with backoff up to a bounded streak; an
/// error with a lost state (`None`) is terminal.
async fn drive<S, P, F>(
    state: S,
    mut poll: P,
    config: TailConfig,
    cancel: CancellationToken,
    mut sink: impl FnMut(LogBatch),
) -> io::Result<()>
where
    P: FnMut(S, PollMode) -> F,
    F: Future<Output = Result<(S, Vec<String>, bool, Option<u64>), (Option<S>, io::Error)>>,
{
    let (window, idle_max) = config.effective();
    // Held in an Option so a failed poll can hand the state back for the
    // retry instead of losing it to the moved-into future.
    let mut slot = Some(state);
    let mut interval = window;
    let mut drain_polls = 0;
    let mut errors = 0;
    loop {
        let mode = if !cancel.is_cancelled() {
            PollMode::Live
        } else {
            drain_polls += 1;
            if drain_polls >= FINAL_DRAIN_CHUNKS {
                PollMode::Last
            } else {
                PollMode::Drain
            }
        };
        let polled = poll(slot.take().expect("state in slot"), mode).await;
        let (next, lines, more, truncated) = match polled {
            Ok(done) => {
                errors = 0;
                done
            }
            Err((Some(prev), error)) if mode == PollMode::Live => {
                errors += 1;
                if errors >= MAX_CONSECUTIVE_POLL_ERRORS {
                    return Err(error);
                }
                slot = Some(prev);
                tracing::warn!(
                    %error,
                    attempt = errors,
                    "log tail poll failed; retrying with backoff"
                );
                interval = interval.saturating_mul(2).min(idle_max);
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => {}
                    () = tokio::time::sleep(interval) => {}
                }
                continue;
            }
            // An error in the drain phase keeps cancellation prompt; a lost
            // state cannot be retried.
            Err((_, error)) => return Err(error),
        };
        slot = Some(next);
        let got_lines = !lines.is_empty();
        if got_lines {
            sink(LogBatch::Lines(lines));
        }
        match mode {
            PollMode::Last => {
                if let Some(bytes) = truncated.filter(|&bytes| bytes > 0) {
                    sink(LogBatch::Truncated { bytes });
                }
                return Ok(());
            }
            PollMode::Drain if !more => return Ok(()),
            // A drain poll with more pending reruns immediately.
            PollMode::Drain => continue,
            PollMode::Live => {
                // Backlog pending: poll again without delay so a writer
                // faster than the window cannot leave lines for the final
                // drain to lose.
                if more {
                    interval = window;
                    continue;
                }
                interval = if got_lines {
                    window
                } else {
                    interval.saturating_mul(2).min(idle_max)
                };
            }
        }
        tokio::select! {
            biased;
            () = cancel.cancelled() => {}
            () = tokio::time::sleep(interval) => {}
        }
    }
}
