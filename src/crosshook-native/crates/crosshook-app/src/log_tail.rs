//! Incremental log-file tailing with per-window line coalescing.
//!
//! [`LogTail`] is a synchronous, offset-tracking reader: each [`LogTail::poll`]
//! reads only the bytes appended since the previous poll and returns the
//! complete lines among them. [`follow`] drives a `LogTail` from async code
//! with adaptive polling and emits at most one [`LogBatch::Lines`] per
//! coalescing window.

use std::fs::File;
use std::future::Future;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

#[cfg(test)]
mod tests;

/// A coalesced chunk of log output delivered to subscribers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogBatch {
    /// Complete lines (without trailing `\n`/`\r\n`) read in one window.
    Lines(Vec<String>),
    /// The subscriber fell behind and `n` earlier **events** were dropped
    /// (broadcast lag). The unit is events, never bytes.
    Dropped {
        /// Number of dropped events.
        n: u64,
    },
    /// The bounded final drain was cut short by [`FINAL_DRAIN_CHUNKS`]: about
    /// `bytes` of log output were left unread. The log file itself is
    /// complete. Never emitted when the estimate is 0.
    Truncated {
        /// Estimated unread **bytes** in the log file.
        bytes: u64,
    },
}

/// Maximum bytes [`LogTail::poll`] reads from disk per call. A larger backlog
/// is consumed over successive polls ([`LogTail::has_backlog`] reports when
/// more may be pending), bounding per-poll memory and batch size.
pub const READ_CHUNK: u64 = 256 * 1024;

/// Maximum complete lines one [`LogTail::poll`] returns. Bytes already read
/// but not yet split into lines stay buffered (bounded by [`READ_CHUNK`]) and
/// are delivered by the following polls, so a flood of tiny lines cannot
/// inflate a single batch (each empty line still costs a `String`).
pub const MAX_LINES_PER_POLL: usize = 4096;

/// Maximum length in bytes of the buffered unterminated line. When an
/// unterminated line grows past this, its first `PARTIAL_CAP` bytes (cut back
/// to a UTF-8 character boundary) are emitted as a line and the remainder
/// stays buffered. A line completed within a single read is not split.
pub const PARTIAL_CAP: usize = 64 * 1024;

/// Read-buffer capacity retained between polls; a larger allocation from a
/// burst is released once reads become small again.
const BUF_RETAIN: usize = 8 * 1024;

/// Incremental reader of a growing log file.
///
/// The file handle stays open between polls. Handles:
/// - **missing file**: polls return no lines until the file appears;
/// - **truncation** (file shorter than the consumed offset): restarts at
///   offset 0 and discards the buffered partial line;
/// - **rotation** (path now refers to a different inode, or was removed):
///   drains the remaining bytes of the old handle (over several polls if
///   needed), then reopens the path at offset 0.
///
/// A truncation followed by a rewrite that is already longer than the old
/// offset before the next poll is indistinguishable from appending and is not
/// detected.
#[derive(Debug)]
pub struct LogTail {
    path: PathBuf,
    file: Option<OpenFile>,
    partial: Vec<u8>,
    /// Bytes read from disk and not yet split into lines; `buf[buf_pos..]` is
    /// the pending part, delivered by later polls once the line budget allows.
    buf: Vec<u8>,
    buf_pos: usize,
    bytes_read: u64,
    /// The last disk read used its whole byte budget, so the file may hold more.
    disk_backlog: bool,
    backlog: bool,
    /// Test seam: serve this many bytes of the next read, then fail.
    #[cfg(test)]
    fault: Option<u64>,
}

#[derive(Debug)]
struct OpenFile {
    file: File,
    ino: u64,
    dev: u64,
    /// Bytes consumed from this handle; the handle's position always equals it.
    offset: u64,
}

impl LogTail {
    /// Creates a tailer for `path`. The file is opened lazily on first poll and
    /// need not exist yet.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            file: None,
            partial: Vec::new(),
            buf: Vec::new(),
            buf_pos: 0,
            bytes_read: 0,
            disk_backlog: false,
            backlog: false,
            #[cfg(test)]
            fault: None,
        }
    }

    /// Path being tailed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Total bytes consumed from disk across all files (each byte counted once).
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Whether the last poll stopped at [`READ_CHUNK`] or
    /// [`MAX_LINES_PER_POLL`], so more lines may already be waiting.
    pub fn has_backlog(&self) -> bool {
        self.backlog
    }

    /// Estimated bytes not yet delivered as lines, measured against the
    /// *current* files: bytes buffered but unsplit, plus what the open handle
    /// has not read yet, plus the whole file at the path if it was rotated
    /// (or not yet opened). Writers can run ahead, so this may undercount.
    fn unread_bytes(&self) -> u64 {
        let pending = (self.buf.len() - self.buf_pos) as u64;
        let path_meta = std::fs::metadata(&self.path).ok();
        let Some(open) = &self.file else {
            return pending + path_meta.map_or(0, |meta| meta.len());
        };
        let on_handle = open
            .file
            .metadata()
            .map_or(0, |meta| meta.len().saturating_sub(open.offset));
        let on_path = path_meta
            .filter(|meta| meta.ino() != open.ino || meta.dev() != open.dev)
            .map_or(0, |meta| meta.len());
        pending + on_handle + on_path
    }

    /// Reads up to [`READ_CHUNK`] bytes appended since the last poll and
    /// returns up to [`MAX_LINES_PER_POLL`] newly completed lines (trailing
    /// `\r` stripped, invalid UTF-8 replaced with U+FFFD). Bytes beyond the
    /// line budget stay buffered for the next poll. An unterminated trailing
    /// line stays buffered (up to [`PARTIAL_CAP`] bytes) until completed or
    /// [`flush_partial`](Self::flush_partial) is called.
    ///
    /// Lines already collected are never discarded by a later failure in the
    /// same poll (e.g. reopening a rotated file): they are returned and the
    /// failed step is retried by the next poll. A read that fails midway keeps
    /// the bytes it did read.
    ///
    /// # Errors
    /// Returns I/O errors other than the file not existing, when no line was
    /// collected by this poll.
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let mut lines = Vec::new();
        match self.poll_into(&mut lines) {
            Ok(()) => Ok(lines),
            Err(error) if !lines.is_empty() => {
                // Those lines are already consumed from disk; failing the
                // poll would lose them. The failed step left its state
                // retryable, so a persistent error resurfaces next poll.
                tracing::warn!(
                    %error,
                    lines = lines.len(),
                    "log tail poll failed after collecting lines; delivering them first"
                );
                Ok(lines)
            }
            Err(error) => Err(error),
        }
    }

    fn poll_into(&mut self, lines: &mut Vec<String>) -> io::Result<()> {
        self.backlog = false;
        let mut budget = READ_CHUNK;
        let path_meta = match std::fs::metadata(&self.path) {
            Ok(meta) => Some(meta),
            Err(err) if err.kind() == io::ErrorKind::NotFound => None,
            Err(err) => return Err(err),
        };

        let rotated = match (&self.file, &path_meta) {
            (Some(open), Some(meta)) => open.ino != meta.ino() || open.dev != meta.dev(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if rotated {
            // Drain whatever the writer appended to the old file, then finish
            // its last line: the old file will never grow again.
            let n = self.read_available(lines, budget)?;
            if self.backlog {
                // Old file not drained yet; keep it and continue next poll.
                return Ok(());
            }
            budget -= n;
            if let Some(line) = self.flush_partial() {
                lines.push(line);
            }
            self.file = None;
            self.disk_backlog = false;
            self.buf.clear();
            self.buf_pos = 0;
        }

        let Some(meta) = path_meta else {
            return Ok(());
        };
        if self.file.is_none() {
            let file = match File::open(&self.path) {
                Ok(file) => file,
                // Removed between metadata() and open(); retry next poll.
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(err) => return Err(err),
            };
            // Use the handle's own metadata so inode and contents agree.
            let opened = file.metadata().unwrap_or(meta);
            self.file = Some(OpenFile {
                file,
                ino: opened.ino(),
                dev: opened.dev(),
                offset: 0,
            });
        } else if let Some(open) = self.file.as_mut() {
            if open.file.metadata()?.len() < open.offset {
                open.file.seek(SeekFrom::Start(0))?;
                open.offset = 0;
                self.partial.clear();
                self.buf.clear();
                self.buf_pos = 0;
                self.disk_backlog = false;
            }
        }
        self.read_available(lines, budget)?;
        Ok(())
    }

    /// Takes the buffered unterminated line, if any (use at end-of-stream).
    pub fn flush_partial(&mut self) -> Option<String> {
        if self.partial.is_empty() {
            return None;
        }
        let line = decode_line(&self.partial);
        self.partial.clear();
        Some(line)
    }

    /// Reads at most `budget` new bytes from the open handle (only when no
    /// bytes are still pending), appends completed lines to `lines` up to
    /// [`MAX_LINES_PER_POLL`] in total, sets `backlog` if either budget ran
    /// out, and returns the number of bytes read from disk.
    ///
    /// The handle's position always equals `offset`: a read that fails after
    /// delivering some bytes keeps and accounts for them, and a read that
    /// delivered none leaves both untouched.
    fn read_available(&mut self, lines: &mut Vec<String>, budget: u64) -> io::Result<u64> {
        let Some(open) = self.file.as_mut() else {
            return Ok(0);
        };
        let mut read = 0;
        if self.buf_pos >= self.buf.len() {
            self.buf.clear();
            self.buf_pos = 0;
            #[cfg(test)]
            let mut faulty;
            #[cfg(test)]
            let reader: &mut dyn Read = match self.fault.take() {
                Some(remaining) => {
                    faulty = tests::FailAfter::new(&mut open.file, remaining);
                    &mut faulty
                }
                None => &mut open.file,
            };
            #[cfg(not(test))]
            let reader: &mut dyn Read = &mut open.file;
            let (n, error) = read_up_to(reader, &mut self.buf, budget);
            if let Some(error) = error {
                if n == 0 {
                    return Err(error);
                }
                tracing::warn!(%error, bytes = n, "log tail read failed midway; keeping the bytes read");
            }
            if self.buf.len() <= BUF_RETAIN && self.buf.capacity() > BUF_RETAIN {
                self.buf.shrink_to(BUF_RETAIN);
            }
            open.offset += n;
            self.bytes_read += n;
            self.disk_backlog = n > 0 && n == budget;
            read = n;
        }
        self.consume_pending(lines);
        self.backlog = self.disk_backlog || self.buf_pos < self.buf.len();
        Ok(read)
    }

    /// Splits pending bytes into lines until `lines` holds
    /// [`MAX_LINES_PER_POLL`]; the rest stays pending for the next poll.
    fn consume_pending(&mut self, lines: &mut Vec<String>) {
        let data = &self.buf[self.buf_pos..];
        let mut pos = 0;
        while lines.len() < MAX_LINES_PER_POLL {
            let Some(nl) = data[pos..].iter().position(|&b| b == b'\n') else {
                buffer_partial(&mut self.partial, &data[pos..], lines);
                pos = data.len();
                break;
            };
            let segment = &data[pos..pos + nl];
            if self.partial.is_empty() {
                lines.push(decode_line(segment));
            } else {
                // Complete the buffered partial line.
                self.partial.extend_from_slice(segment);
                lines.push(decode_line(&self.partial));
                self.partial.clear();
            }
            pos += nl + 1;
        }
        self.buf_pos += pos;
    }
}

/// Appends up to `budget` bytes from `reader` to `buf`. Returns the bytes
/// appended and the error that cut the read short, if any; bytes read before
/// an error stay in `buf` and are counted.
fn read_up_to(reader: &mut dyn Read, buf: &mut Vec<u8>, budget: u64) -> (u64, Option<io::Error>) {
    let before = buf.len();
    let result = reader.take(budget).read_to_end(buf);
    ((buf.len() - before) as u64, result.err())
}

/// Appends `bytes` to the unterminated-line buffer, emitting capped prefixes
/// while it exceeds [`PARTIAL_CAP`].
fn buffer_partial(partial: &mut Vec<u8>, bytes: &[u8], lines: &mut Vec<String>) {
    partial.extend_from_slice(bytes);
    while partial.len() > PARTIAL_CAP {
        let cut = char_boundary_at_or_before(partial, PARTIAL_CAP);
        lines.push(String::from_utf8_lossy(&partial[..cut]).into_owned());
        partial.drain(..cut);
    }
}

/// Largest index `<= at` that does not split a UTF-8 sequence. Falls back to
/// `at` when the bytes there are not a valid sequence tail (invalid input is
/// replaced on decode anyway). Requires `at < bytes.len()`.
fn char_boundary_at_or_before(bytes: &[u8], at: usize) -> usize {
    let is_continuation = |b: u8| b & 0xC0 == 0x80;
    let mut cut = at;
    // A UTF-8 sequence has at most three continuation bytes.
    while cut > 0 && at - cut < 3 && is_continuation(bytes[cut]) {
        cut -= 1;
    }
    if is_continuation(bytes[cut]) || cut == 0 {
        at
    } else {
        cut
    }
}

fn decode_line(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

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
/// poll reads at most [`READ_CHUNK`] bytes and [`MAX_LINES_PER_POLL`] lines,
/// so the drain reaches 16 MiB of long lines (64 × [`READ_CHUNK`]) or about
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
/// resets it. All lines read by one poll (at most [`READ_CHUNK`] bytes and
/// [`MAX_LINES_PER_POLL`] lines) form one [`LogBatch::Lines`]; a larger
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
    let poll = |tail: LogTail, mode: PollMode| blocking_poll(tail, mode, poll_once);
    drive(LogTail::new(path), poll, config, cancel, sink).await
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
