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
    /// The subscriber fell behind and `n` earlier events were dropped.
    Dropped {
        /// Number of dropped events.
        n: u64,
    },
}

/// Maximum bytes [`LogTail::poll`] reads from disk per call. A larger backlog
/// is consumed over successive polls ([`LogTail::has_backlog`] reports when
/// more may be pending), bounding per-poll memory and batch size.
pub const READ_CHUNK: u64 = 256 * 1024;

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
    buf: Vec<u8>,
    bytes_read: u64,
    backlog: bool,
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
            bytes_read: 0,
            backlog: false,
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

    /// Whether the last poll stopped at [`READ_CHUNK`], so more bytes may
    /// already be waiting on disk.
    pub fn has_backlog(&self) -> bool {
        self.backlog
    }

    /// Reads up to [`READ_CHUNK`] bytes appended since the last poll and
    /// returns the newly completed lines (trailing `\r` stripped, invalid
    /// UTF-8 replaced with U+FFFD). An unterminated trailing line stays
    /// buffered (up to [`PARTIAL_CAP`] bytes) until completed or
    /// [`flush_partial`](Self::flush_partial) is called.
    ///
    /// # Errors
    /// Returns I/O errors other than the file not existing.
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let mut lines = Vec::new();
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
            let n = self.read_available(&mut lines, budget)?;
            if self.backlog {
                // Old file not drained yet; keep it and continue next poll.
                return Ok(lines);
            }
            budget -= n;
            if let Some(line) = self.flush_partial() {
                lines.push(line);
            }
            self.file = None;
        }

        let Some(meta) = path_meta else {
            return Ok(lines);
        };
        if self.file.is_none() {
            let file = match File::open(&self.path) {
                Ok(file) => file,
                // Removed between metadata() and open(); retry next poll.
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(lines),
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
            }
        }
        self.read_available(&mut lines, budget)?;
        Ok(lines)
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

    /// Reads at most `budget` bytes from the open handle, appends completed
    /// lines to `lines`, sets `backlog` if the budget was exhausted, and
    /// returns the number of bytes read.
    fn read_available(&mut self, lines: &mut Vec<String>, budget: u64) -> io::Result<u64> {
        let Some(open) = self.file.as_mut() else {
            return Ok(0);
        };
        self.buf.clear();
        let n = (&mut open.file).take(budget).read_to_end(&mut self.buf)? as u64;
        if self.buf.len() <= BUF_RETAIN && self.buf.capacity() > BUF_RETAIN {
            self.buf.shrink_to(BUF_RETAIN);
        }
        if n == 0 {
            return Ok(0);
        }
        self.backlog = n == budget;
        open.offset += n;
        self.bytes_read += n;

        let mut chunk: &[u8] = &self.buf;
        // Complete the buffered partial line first, if this chunk ends it.
        if !self.partial.is_empty() {
            match chunk.iter().position(|&b| b == b'\n') {
                Some(nl) => {
                    self.partial.extend_from_slice(&chunk[..nl]);
                    lines.push(decode_line(&self.partial));
                    self.partial.clear();
                    chunk = &chunk[nl + 1..];
                }
                None => {
                    buffer_partial(&mut self.partial, chunk, lines);
                    return Ok(n);
                }
            }
        }
        while let Some(nl) = chunk.iter().position(|&b| b == b'\n') {
            lines.push(decode_line(&chunk[..nl]));
            chunk = &chunk[nl + 1..];
        }
        buffer_partial(&mut self.partial, chunk, lines);
        Ok(n)
    }
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

/// Maximum polls [`follow`] runs after cancellation to drain a backlog
/// (16 × [`READ_CHUNK`] = 4 MiB), so cancellation stays prompt even when a
/// writer outpaces the reader. Bytes still pending afterwards are not read.
pub const FINAL_DRAIN_CHUNKS: u32 = 16;

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

/// Tails `path` until `cancel` fires, delivering coalesced lines to `sink`.
///
/// Polls once immediately, then every coalescing window while data flows;
/// each empty poll doubles the interval up to `idle_backoff_max`, and new
/// data resets it. All lines read by one poll (at most [`READ_CHUNK`] bytes)
/// form one [`LogBatch::Lines`]; a larger backlog is delivered over the
/// following windows. Empty batches are never emitted. On cancellation the
/// remaining bytes are read without waiting, up to [`FINAL_DRAIN_CHUNKS`]
/// polls; then the unterminated trailing line (if any) is flushed and the
/// function returns. File reads run on Tokio's blocking pool, so `sink`
/// should be non-blocking.
///
/// # Errors
/// Returns the first I/O error from reading the file, or an error if the
/// blocking read task panicked.
pub async fn follow(
    path: impl Into<PathBuf>,
    config: TailConfig,
    cancel: CancellationToken,
    sink: impl FnMut(LogBatch),
) -> io::Result<()> {
    let poll = |mut tail: LogTail, mode: PollMode| async move {
        let (tail, result) = tokio::task::spawn_blocking(move || {
            let result = tail.poll().map(|mut lines| {
                let more = tail.has_backlog();
                if mode == PollMode::Last || (mode == PollMode::Drain && !more) {
                    lines.extend(tail.flush_partial());
                }
                (lines, more)
            });
            (tail, result)
        })
        .await
        .map_err(io::Error::other)?;
        let (lines, more) = result?;
        Ok((tail, lines, more))
    };
    drive(LogTail::new(path), poll, config, cancel, sink).await
}

/// Time-driven core of [`follow`], generic over the poll so the cadence can
/// be tested deterministically. The poll takes the state by value and hands
/// it back with the lines read and whether more bytes are already pending.
/// After cancellation it is called with [`PollMode::Drain`] without delay
/// while it reports more pending, at most [`FINAL_DRAIN_CHUNKS`] times in
/// total, the last of which uses [`PollMode::Last`].
async fn drive<S, P, F>(
    mut state: S,
    mut poll: P,
    config: TailConfig,
    cancel: CancellationToken,
    mut sink: impl FnMut(LogBatch),
) -> io::Result<()>
where
    P: FnMut(S, PollMode) -> F,
    F: Future<Output = io::Result<(S, Vec<String>, bool)>>,
{
    let (window, idle_max) = config.effective();
    let mut interval = window;
    let mut drain_polls = 0;
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
        let (next, lines, more) = poll(state, mode).await?;
        state = next;
        let got_lines = !lines.is_empty();
        if got_lines {
            sink(LogBatch::Lines(lines));
        }
        match mode {
            PollMode::Last => return Ok(()),
            PollMode::Drain if !more => return Ok(()),
            PollMode::Drain => continue,
            PollMode::Live => {}
        }
        interval = if more || got_lines {
            window
        } else {
            interval.saturating_mul(2).min(idle_max)
        };
        tokio::select! {
            biased;
            () = cancel.cancelled() => {}
            () = tokio::time::sleep(interval) => {}
        }
    }
}
