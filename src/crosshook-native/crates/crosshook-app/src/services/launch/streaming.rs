//! Child stdio tee into the launch log and the log-follow driver (Task 2.3).
//!
//! One streaming path serves both spawn modes. When the child's stdout or
//! stderr is piped, a lossy byte tee drains both pipes into the log file
//! (one writer task, bounded channel, [`PARTIAL_CAP`] partial-line cap) so a
//! pipe is never left undrained — even on invalid UTF-8, where the old
//! line-based reader used to stop (D7). Whether or not a tee is needed,
//! [`follow`] tails the log file and delivers coalesced [`LogBatch`]es to the
//! sink, replacing the 500 ms whole-file re-read (D3). Neither path ever
//! kills the child: it exits on its own, or not at all.

use std::io;
use std::path::Path;
use std::process::ExitStatus;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::types::EventPublisher;
use crate::log_tail::{follow, LogBatch, TailConfig, PARTIAL_CAP};

/// Pipe-reader buffer budget and channel capacity, shared by both readers.
const PIPE_BUF_CAPACITY: usize = 8 * 1024;
/// Bounded chunks in flight between the pipe readers and the single writer.
const PIPE_CHANNEL_CAPACITY: usize = 256;

/// Streams `child`'s output into `log_path` and to `sink`, then returns the
/// exit status.
///
/// If `child.stdout` or `child.stderr` is piped, one reader per pipe drains
/// raw bytes (decoding is deferred and lossy) and a single writer appends the
/// chunks to `log_path` in arrival order, so both streams interleave as one
/// log. A chunk longer than [`PARTIAL_CAP`] is split into capped pieces; a
/// write failure stops file writes but never the draining, so the child can
/// never stall on a full pipe. In every mode [`follow`] runs concurrently,
/// delivering coalesced, lossily decoded lines to `sink`.
///
/// drain and partial-line flush deliver the last lines before this function
/// returns. The child is never killed; with `kill_on_drop` configured, a
/// still-running child is left to the caller's `Child` drop semantics.
///
/// The log file must already exist (the service creates it via
/// `create_log_path`); this function opens it append-mode and fails fast
/// otherwise. `sink` must be non-blocking — `EventPublisher::log` is a
/// bounded `broadcast::send`.
///
/// # Errors
/// Returns the I/O error of `child.wait()` or of following the log file. Pipe
/// tee errors (open, write, reader) are logged and absorbed.
pub async fn stream_child_log(
    child: &mut Child,
    log_path: &Path,
    sink: impl FnMut(LogBatch) + Send,
) -> io::Result<ExitStatus> {
    let piped = child.stdout.is_some() || child.stderr.is_some();
    let writer = if piped {
        Some(spawn_pipe_tee(child, log_path).await)
    } else {
        None
    };

    let tail_cancel = CancellationToken::new();
    let tail = follow(
        log_path.to_path_buf(),
        TailConfig::default(),
        tail_cancel.clone(),
        sink,
    );
    // Drive the tail concurrently with the child so lines stream live; once
    // the child has exited and the tee has flushed, cancel the tail so its
    // bounded final drain and partial flush run before we return.
    let waiter = async {
        let status = child.wait().await;
        if let Some(writer) = writer {
            writer.finish().await;
        }
        tail_cancel.cancel();
        status
    };
    let (status, tail_result) = tokio::join!(waiter, tail);
    // A wait error means there is no status; surface it first. A tail error
    // still lets the session finish on the real exit status.
    let status = status?;
    if let Err(error) = tail_result {
        tracing::warn!(%error, path = %log_path.display(), "launch log follow failed");
    }
    Ok(status)
}

/// [`stream_child_log`] with the session publisher as the sink (I-40).
///
/// An I/O error yields `None` so the finalizer tears the session down with an
/// unknown exit status rather than failing the whole finalization.
pub(super) async fn stream_session(
    child: Child,
    log_path: &Path,
    publisher: &EventPublisher,
) -> Option<ExitStatus> {
    let mut child = child;
    let publisher = publisher.clone();
    stream_child_log(&mut child, log_path, move |batch| publisher.log(batch))
        .await
        .ok()
}

/// The running pipe tee: join handles of the per-pipe readers plus the single
/// log writer. `finish` waits for every reader to hit EOF, then drains and
/// flushes the writer.
struct PipeTee {
    readers: Vec<tokio::task::JoinHandle<()>>,
    writer: tokio::task::JoinHandle<()>,
}

impl PipeTee {
    /// Waits out the tee: readers to EOF, writer drained and flushed. Failures
    /// are logged; the tee must never make a launch fail.
    async fn finish(self) {
        for reader in self.readers {
            if let Err(error) = reader.await {
                tracing::warn!(%error, "launch log pipe reader task failed");
            }
        }
        if let Err(error) = self.writer.await {
            tracing::warn!(%error, "launch log writer task failed");
        }
    }
}

/// Spawns one reader per piped stream plus the single append-mode writer, and
/// returns the running tee.
async fn spawn_pipe_tee(child: &mut Child, log_path: &Path) -> PipeTee {
    let (tx, rx) = mpsc::channel::<Vec<u8>>(PIPE_CHANNEL_CAPACITY);
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tokio::spawn(pipe_reader(stdout, tx.clone())));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tokio::spawn(pipe_reader(stderr, tx.clone())));
    }
    drop(tx);
    let writer = tokio::spawn(log_writer(log_path.to_path_buf(), rx));
    PipeTee { readers, writer }
}

/// Drains one pipe into `tx` until EOF.
///
/// Reads bytes with `read_until(b'\n')`; a chunk that grows past
/// [`PARTIAL_CAP`] (an over-long line) is flushed as capped pieces and the
/// buffer is reused. Decoding is deferred to the writer and the tail, both of
/// which are lossy, so invalid UTF-8 never stops the drain (D7). The child is
/// never killed; if the channel closes, draining continues to EOF without
/// sending.
async fn pipe_reader<R>(pipe: R, tx: mpsc::Sender<Vec<u8>>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut pipe = tokio::io::BufReader::with_capacity(PIPE_BUF_CAPACITY, pipe);
    let mut chunk: Vec<u8> = Vec::with_capacity(PIPE_BUF_CAPACITY);
    let mut dead = false;
    loop {
        chunk.clear();
        match pipe.read_until(b'\n', &mut chunk).await {
            Ok(0) => return,
            Ok(_) => {
                if dead {
                    continue;
                }
                // Split over-long chunks so no tee write exceeds PARTIAL_CAP;
                // every piece except the last ends with '\n', which keeps
                // tail lines newline-complete and preserves the cap (D7).
                while chunk.len() > PARTIAL_CAP {
                    let mut piece: Vec<u8> = chunk.drain(..PARTIAL_CAP - 1).collect();
                    piece.push(b'\n');
                    chunk.shrink_to(PIPE_BUF_CAPACITY);
                    if tx.send(piece).await.is_err() {
                        dead = true;
                        break;
                    }
                }
                if !dead && !chunk.is_empty() && tx.send(std::mem::take(&mut chunk)).await.is_err()
                {
                    // Writer gone: keep draining to EOF so the child never
                    // blocks on a full pipe, just stop sending.
                    dead = true;
                }
            }
            Err(_) => return,
        }
    }
}

/// Appends every received chunk to the log file, in arrival order, until all
/// senders are gone (readers at EOF); then flushes. Open, write or flush
/// failures are logged and absorbed: after a write failure the channel is
/// still drained to the end so the readers are never blocked.
async fn log_writer(path: std::path::PathBuf, mut rx: mpsc::Receiver<Vec<u8>>) {
    let mut file = match OpenOptions::new().append(true).open(&path).await {
        Ok(file) => Some(file),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to open pipe-backed launch log");
            None
        }
    };
    while let Some(chunk) = rx.recv().await {
        if let Some(open) = file.as_mut() {
            if let Err(error) = open.write_all(&chunk).await {
                tracing::warn!(%error, path = %path.display(), "failed to append to pipe-backed launch log");
                file = None;
            }
        }
        // After a write failure the channel is still drained to the end so
        // the readers — and therefore the child's pipes — never block.
    }
    if let Some(mut file) = file {
        if let Err(error) = file.flush().await {
            tracing::warn!(%error, path = %path.display(), "failed to flush pipe-backed launch log");
        }
    }
}
