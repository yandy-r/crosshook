//! Child stdio tee into the launch log and the log-follow driver (Task 2.3).
//!
//! One streaming path serves both spawn modes. When the child's stdout or
//! stderr is piped, the shared lossy byte tee
//! ([`spawn_pipe_tee`], `services/process/tee.rs`) drains both pipes into the
//! log file (one writer task, bounded channel,
//! [`PARTIAL_CAP`](crate::log_tail::PARTIAL_CAP)-bounded reads) so a pipe is
//! never left undrained — even on invalid UTF-8, where the old line-based
//! reader used to stop (D7). Whether or not a tee is needed, [`follow`] tails
//! the log file and delivers coalesced [`LogBatch`]es to the sink, replacing
//! the 500 ms whole-file re-read (D3). Neither path ever kills the child: it
//! exits on its own, or not at all.

use std::io;
use std::path::Path;
use std::process::ExitStatus;

use tokio::process::Child;
use tokio_util::sync::CancellationToken;

use super::types::EventPublisher;
use crate::log_tail::{follow, LogBatch, TailConfig};
use crate::services::process::tee::spawn_pipe_tee;

/// Streams `child`'s output into `log_path` and to `sink`, then returns the
/// exit status.
///
/// If `child.stdout` or `child.stderr` is piped, one reader per pipe drains
/// raw bytes (decoding is deferred and lossy) and a single writer appends the
/// chunks to `log_path` in arrival order, so both streams interleave as one
/// log. Reads are bounded to [`PARTIAL_CAP`](crate::log_tail::PARTIAL_CAP) and forwarded byte-exact, so
/// even a newline-free stream flows in capped pieces; a write failure stops
/// file writes but never the draining, so the child can never stall on a
/// full pipe. In every mode [`follow`] runs concurrently, delivering
/// coalesced, lossily decoded lines to `sink`.
///
/// On exit the tee is flushed and the tail runs its bounded final drain and
/// returns. The child is never killed; with `kill_on_drop` configured, a
/// still-running child is left to the caller's `Child` drop semantics.
///
/// The log file must already exist (the service creates it via
/// `create_log_path`); this function opens it append-mode and fails fast
/// otherwise. `sink` must be non-blocking — `EventPublisher::log` is a
/// bounded `broadcast::send`.
///
/// # Errors
/// Returns the I/O error of `child.wait()`. A failure of the log follow
/// (poll errors past the retry streak, or a panicked or cancelled blocking
/// read task) is logged and does not fail the call: the real exit status is
/// still returned. Pipe tee errors (open, write, reader) are likewise logged
/// and absorbed.
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
    match stream_child_log(&mut child, log_path, move |batch| publisher.log(batch)).await {
        Ok(status) => Some(status),
        Err(error) => {
            tracing::warn!(%error, "launch log streaming failed; exit status unknown");
            None
        }
    }
}
