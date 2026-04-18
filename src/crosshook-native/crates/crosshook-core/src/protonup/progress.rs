//! Progress event types and emitter for the Proton install orchestrator.

use tokio::sync::broadcast;

/// Bounded backlog for progress snapshots sent to UI listeners.
///
/// Progress events are best-effort status updates, not an audit log. The
/// channel is intentionally lossy under back-pressure so a stalled UI cannot
/// grow unbounded memory while download/verify/extract work continues. Lagging
/// receivers should expect `broadcast::error::RecvError::Lagged` and treat the
/// next snapshot as the current install state.
const PROGRESS_EVENT_BUFFER_CAP: usize = 64;

/// Install lifecycle phase.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Resolving,
    Downloading,
    Verifying,
    Extracting,
    Finalizing,
    Done,
    Failed,
    Cancelled,
}

/// One progress snapshot emitted during an install operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonInstallProgress {
    pub op_id: String,
    pub phase: Phase,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub message: Option<String>,
}

/// Emitter handle.  Clone-cheap; backed by a `broadcast` channel.
#[derive(Clone)]
pub struct ProgressEmitter {
    op_id: String,
    tx: broadcast::Sender<ProtonInstallProgress>,
}

impl ProgressEmitter {
    /// Create a new emitter and its paired receiver.
    pub fn new(op_id: impl Into<String>) -> (Self, broadcast::Receiver<ProtonInstallProgress>) {
        let (tx, rx) = broadcast::channel(PROGRESS_EVENT_BUFFER_CAP);
        (
            Self {
                op_id: op_id.into(),
                tx,
            },
            rx,
        )
    }

    /// Subscribe to receive future events from this emitter.
    ///
    /// The bounded channel only retains the most recent
    /// [`PROGRESS_EVENT_BUFFER_CAP`] snapshots. Slow consumers may receive
    /// `broadcast::error::RecvError::Lagged` and should continue with the next
    /// available snapshot.
    pub fn subscribe(&self) -> broadcast::Receiver<ProtonInstallProgress> {
        self.tx.subscribe()
    }

    pub fn op_id(&self) -> &str {
        &self.op_id
    }

    /// Send a progress snapshot. Silently ignores the case where no receivers are listening.
    pub fn emit(
        &self,
        phase: Phase,
        bytes_done: u64,
        bytes_total: Option<u64>,
        message: Option<String>,
    ) {
        let _ = self.tx.send(ProtonInstallProgress {
            op_id: self.op_id.clone(),
            phase,
            bytes_done,
            bytes_total,
            message,
        });
    }
}
