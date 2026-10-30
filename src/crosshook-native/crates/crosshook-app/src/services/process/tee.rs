//! Lossy, bounded pipe tee: drains a child's stdout and stderr into one log
//! file (moved from `services/launch/streaming.rs`).
//!
//! One reader per piped stream forwards raw bytes through a bounded channel
//! to a single writer, so both streams interleave in arrival order. Reads are
//! capped at [`PARTIAL_CAP`] and forwarded byte-exact, so neither invalid
//! UTF-8 nor a newline-free flood can stop the drain (launch D7). A write
//! failure stops file writes but never the draining, so the child can never
//! stall on a full pipe. The tee never kills the child.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::log_tail::PARTIAL_CAP;

/// Pipe-reader buffer budget and channel capacity, shared by both readers.
const PIPE_BUF_CAPACITY: usize = 8 * 1024;
/// Bounded chunks in flight between the pipe readers and the single writer.
const PIPE_CHANNEL_CAPACITY: usize = 256;

/// The running pipe tee: join handles of the per-pipe readers plus the single
/// log writer.
pub(crate) struct PipeTee {
    readers: Vec<JoinHandle<()>>,
    writer: JoinHandle<()>,
}

impl PipeTee {
    /// Waits out the tee: readers to EOF, writer drained and flushed. Failures
    /// are logged; the tee must never make a run fail.
    pub(crate) async fn finish(self) {
        join_readers(self.readers).await;
        join_writer(self.writer).await;
    }

    /// Like [`finish`](Self::finish), but the readers get at most `grace`
    /// to reach EOF and are abandoned at once when `cancel` fires.
    ///
    /// A descendant that outlives the supervised group (or left it) can hold
    /// the pipes open forever, so EOF is not guaranteed after the leader
    /// exits. Abandoned readers are aborted, which closes the read ends; the
    /// writer then flushes everything already received. Returns whether
    /// every reader reached EOF.
    pub(crate) async fn finish_within(self, grace: Duration, cancel: &CancellationToken) -> bool {
        let aborts: Vec<_> = self.readers.iter().map(JoinHandle::abort_handle).collect();
        let drained = tokio::select! {
            biased;
            () = join_readers(self.readers) => true,
            () = cancel.cancelled() => false,
            () = tokio::time::sleep(grace) => false,
        };
        if !drained {
            for abort in aborts {
                abort.abort();
            }
        }
        // Every reader is done or aborted, so every sender is gone and the
        // writer ends after the chunks already queued.
        join_writer(self.writer).await;
        drained
    }
}

/// Spawns one reader per piped stream plus the single writer, which opens
/// the existing `log_path` append-mode, and returns the running tee.
pub(crate) async fn spawn_pipe_tee(child: &mut Child, log_path: &Path) -> PipeTee {
    spawn_tee(child, log_path, LogTarget::Append)
}

/// [`spawn_pipe_tee`] writing through an already open handle of `log_path`
/// (the one returned by `create_log_file_in`), so the log is never reopened
/// by path. `log_path` only labels diagnostics.
pub(crate) fn spawn_pipe_tee_to_file(
    child: &mut Child,
    log_path: &Path,
    file: std::fs::File,
) -> PipeTee {
    spawn_tee(child, log_path, LogTarget::Handle(file))
}

/// Where the tee writer appends.
enum LogTarget {
    /// Open the existing log file by path, append-mode.
    Append,
    /// Write through this open handle.
    Handle(std::fs::File),
}

fn spawn_tee(child: &mut Child, log_path: &Path, target: LogTarget) -> PipeTee {
    let (tx, rx) = mpsc::channel::<Vec<u8>>(PIPE_CHANNEL_CAPACITY);
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tokio::spawn(pipe_reader(stdout, tx.clone())));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tokio::spawn(pipe_reader(stderr, tx.clone())));
    }
    drop(tx);
    let writer = tokio::spawn(log_writer(log_path.to_path_buf(), target, rx));
    PipeTee { readers, writer }
}

async fn join_readers(readers: Vec<JoinHandle<()>>) {
    for reader in readers {
        if let Err(error) = reader.await {
            tracing::warn!(%error, "process log pipe reader task failed");
        }
    }
}

async fn join_writer(writer: JoinHandle<()>) {
    if let Err(error) = writer.await {
        tracing::warn!(%error, "process log writer task failed");
    }
}

/// Drains one pipe into `tx` until EOF.
///
/// Each read is bounded to [`PARTIAL_CAP`] bytes via `take`, so a
/// newline-free stream is forwarded as capped raw pieces instead of being
/// buffered whole in memory. Pieces are written through byte-exact — no
/// separator is inserted — and decoding plus partial-line capping are
/// deferred to the tail, which is lossy, so invalid UTF-8 never stops the
/// drain. If the channel closes, draining continues to EOF without sending.
async fn pipe_reader<R>(pipe: R, tx: mpsc::Sender<Vec<u8>>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut pipe = tokio::io::BufReader::with_capacity(PIPE_BUF_CAPACITY, pipe);
    let mut chunk: Vec<u8> = Vec::with_capacity(PIPE_BUF_CAPACITY);
    let mut dead = false;
    loop {
        chunk.clear();
        match (&mut pipe)
            .take(PARTIAL_CAP as u64)
            .read_until(b'\n', &mut chunk)
            .await
        {
            Ok(0) => return,
            Ok(_) => {
                if dead || chunk.is_empty() {
                    continue;
                }
                if tx.send(std::mem::take(&mut chunk)).await.is_err() {
                    // Writer gone: keep draining to EOF so the child never
                    // blocks on a full pipe, just stop sending.
                    dead = true;
                }
            }
            Err(error) => {
                // The pipe is unreadable from here on; the tee for this
                // stream ends, so say so instead of silently losing output.
                tracing::warn!(%error, "process log pipe read failed; this stream is no longer teed");
                return;
            }
        }
    }
}

/// Appends every received chunk to the log, in arrival order, until all
/// senders are gone (readers at EOF or aborted); then flushes. Open, write or
/// flush failures are logged and absorbed: after a write failure the channel
/// is still drained to the end so the readers are never blocked.
async fn log_writer(path: PathBuf, target: LogTarget, mut rx: mpsc::Receiver<Vec<u8>>) {
    let mut file = match target {
        LogTarget::Handle(file) => Some(tokio::fs::File::from_std(file)),
        LogTarget::Append => match OpenOptions::new().append(true).open(&path).await {
            Ok(file) => Some(file),
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "failed to open pipe-backed process log");
                None
            }
        },
    };
    while let Some(chunk) = rx.recv().await {
        if let Some(open) = file.as_mut() {
            if let Err(error) = open.write_all(&chunk).await {
                tracing::warn!(%error, path = %path.display(), "failed to append to pipe-backed process log");
                file = None;
            }
        }
        // After a write failure the channel is still drained to the end so
        // the readers — and therefore the child's pipes — never block.
    }
    if let Some(mut file) = file {
        if let Err(error) = file.flush().await {
            tracing::warn!(%error, path = %path.display(), "failed to flush pipe-backed process log");
        }
    }
}
