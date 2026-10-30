//! Incremental log-file tailing with per-window line coalescing.
//!
//! [`LogTail`] is a synchronous, offset-tracking reader: each [`LogTail::poll`]
//! reads only the bytes appended since the previous poll and returns the
//! complete lines among them. [`follow`] drives a `LogTail` from async code
//! with adaptive polling and emits at most one [`LogBatch::Lines`] per
//! coalescing window.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use nix::libc;

mod driver;
#[cfg(test)]
mod tests;

pub use driver::{follow, follow_file, TailConfig, FINAL_DRAIN_CHUNKS};

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
/// The path is opened once, lazily, with `O_NOFOLLOW | O_NONBLOCK`, and only
/// a regular file is accepted: a symlink or FIFO planted at the path is an
/// error, never followed and never blocked on. That first file is the whole
/// log: its handle stays open between polls and the path is never reopened.
/// Handles:
/// - **missing file**: polls return no lines until the file appears;
/// - **truncation** (file shorter than the consumed offset): restarts at
///   offset 0 and discards the buffered partial line;
/// - **replacement** (the path now names a different inode — rotated,
///   swapped for a symlink or FIFO — or was removed): drains the remaining
///   bytes of the open handle (over several polls if needed), flushes its
///   last line and ends the log. Whatever the path names afterwards is never
///   read.
///
/// A truncation followed by a rewrite that is already longer than the old
/// offset before the next poll is indistinguishable from appending and is not
/// detected.
#[derive(Debug)]
pub struct LogTail {
    path: PathBuf,
    /// Identity the file at `path` must have to be read; `None` accepts the
    /// first regular file found there.
    pin: Option<FileId>,
    file: Option<OpenFile>,
    /// The log ended (its file was replaced or refused); nothing more is read.
    ended: bool,
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

/// Device and inode of a file: what "the same file" means for the tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileId {
    dev: u64,
    ino: u64,
}

impl FileId {
    fn of(meta: &std::fs::Metadata) -> Self {
        Self {
            dev: meta.dev(),
            ino: meta.ino(),
        }
    }
}

#[derive(Debug)]
struct OpenFile {
    file: File,
    id: FileId,
    /// Bytes consumed from this handle; the handle's position always equals it.
    offset: u64,
}

impl LogTail {
    /// Creates a tailer for `path`. The file is opened lazily on first poll and
    /// need not exist yet.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            pin: None,
            file: None,
            ended: false,
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

    /// Creates a tailer for `path` that reads only the file `created` refers
    /// to (for example the handle a log was created with): if the path names
    /// any other file when it is first opened, the log ends unread.
    ///
    /// # Errors
    /// The `fstat` error of `created`.
    pub fn pinned(path: impl Into<PathBuf>, created: &File) -> io::Result<Self> {
        let pin = FileId::of(&created.metadata()?);
        Ok(Self {
            pin: Some(pin),
            ..Self::new(path)
        })
    }

    /// Path being tailed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Total bytes consumed from disk (each byte counted once).
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Whether the last poll stopped at [`READ_CHUNK`] or
    /// [`MAX_LINES_PER_POLL`], so more lines may already be waiting.
    pub fn has_backlog(&self) -> bool {
        self.backlog
    }

    /// Estimated bytes not yet delivered as lines: bytes buffered but
    /// unsplit, plus what the open handle has not read yet, or — before the
    /// first open — the regular file at the path. A replaced path is never
    /// read, so it is not counted. Writers can run ahead, so this may
    /// undercount.
    fn unread_bytes(&self) -> u64 {
        let pending = (self.buf.len() - self.buf_pos) as u64;
        if self.ended {
            return pending;
        }
        let Some(open) = &self.file else {
            let at_path = std::fs::symlink_metadata(&self.path)
                .ok()
                .filter(|meta| meta.file_type().is_file())
                .map_or(0, |meta| meta.len());
            return pending + at_path;
        };
        let on_handle = open
            .file
            .metadata()
            .map_or(0, |meta| meta.len().saturating_sub(open.offset));
        pending + on_handle
    }

    /// Reads up to [`READ_CHUNK`] bytes appended since the last poll and
    /// returns up to [`MAX_LINES_PER_POLL`] newly completed lines (trailing
    /// `\r` stripped, invalid UTF-8 replaced with U+FFFD). Bytes beyond the
    /// line budget stay buffered for the next poll. An unterminated trailing
    /// line stays buffered (up to [`PARTIAL_CAP`] bytes) until completed or
    /// [`flush_partial`](Self::flush_partial) is called.
    ///
    /// Lines already collected are never discarded by a later failure in the
    /// same poll: they are returned and the failed step is retried by the
    /// next poll. A read that fails midway keeps the bytes it did read.
    ///
    /// # Errors
    /// Returns I/O errors other than the file not existing, when no line was
    /// collected by this poll: e.g. an unpinned tail finding a symlink
    /// (`ELOOP`) or a non-regular file at the path.
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
        if self.ended {
            return Ok(());
        }
        // lstat: a symlink swapped in is a different inode, never its target.
        let path_meta = match std::fs::symlink_metadata(&self.path) {
            Ok(meta) => Some(meta),
            Err(err) if err.kind() == io::ErrorKind::NotFound => None,
            Err(err) => return Err(err),
        };

        if let Some(open) = self.file.as_mut() {
            let replaced = path_meta
                .as_ref()
                .is_none_or(|meta| FileId::of(meta) != open.id);
            if replaced {
                // Drain whatever the writer appended to the open file, then
                // finish its last line and end the log: the path is never
                // reopened, so a file swapped in is never read.
                self.read_available(lines, READ_CHUNK)?;
                if self.backlog {
                    // Not drained yet; continue next poll.
                    return Ok(());
                }
                lines.extend(self.flush_partial());
                self.end();
                return Ok(());
            }
            if open.file.metadata()?.len() < open.offset {
                open.file.seek(SeekFrom::Start(0))?;
                open.offset = 0;
                self.partial.clear();
                self.buf.clear();
                self.buf_pos = 0;
                self.disk_backlog = false;
            }
        } else if path_meta.is_none() || !self.open()? {
            return Ok(());
        }
        self.read_available(lines, READ_CHUNK)?;
        Ok(())
    }

    /// Opens the path without following a symlink or blocking on a FIFO.
    /// Returns whether a file is now open. A pinned tail ends the log when
    /// the path cannot be opened (other than not existing) or names another
    /// file; an unpinned one reports an error for a non-regular file.
    fn open(&mut self) -> io::Result<bool> {
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.path);
        let file = match opened {
            Ok(file) => file,
            // Removed since the lstat; retry next poll.
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) if self.pin.is_some() => {
                tracing::warn!(%error, "log file cannot be reopened; ending the log");
                self.end();
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let meta = file.metadata()?;
        let id = FileId::of(&meta);
        if self.pin.is_some_and(|pin| pin != id) {
            tracing::warn!("log path no longer names the created log file; ending the log");
            self.end();
            return Ok(false);
        }
        if !meta.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "log path is not a regular file",
            ));
        }
        self.file = Some(OpenFile {
            file,
            id,
            offset: 0,
        });
        Ok(true)
    }

    /// Ends the log: drops the handle and every buffered byte not yet split.
    fn end(&mut self) {
        self.ended = true;
        self.file = None;
        self.disk_backlog = false;
        self.buf.clear();
        self.buf_pos = 0;
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
