//! Progress consumption and relay helpers.

use crosshook_core::protonup::progress::{Phase, ProgressEmitter, ProtonInstallProgress};
use tokio::sync::broadcast::{self, error::RecvError};

use super::messages;

/// Next progress snapshot, or `None` once the sender side is gone.
///
/// A lagging receiver skips to the next retained snapshot, which is the
/// current install state; progress is best-effort, not an audit log.
pub async fn next_progress(
    rx: &mut broadcast::Receiver<ProtonInstallProgress>,
) -> Option<ProtonInstallProgress> {
    loop {
        match rx.recv().await {
            Ok(progress) => return Some(progress),
            Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return None,
        }
    }
}

/// Relays every core progress event to `out` with its message sanitized by
/// [`messages::progress_message`].
///
/// Returns `true` when a terminal (`Done`, `Failed` or `Cancelled`) phase was
/// relayed. Ends when the core sender is dropped.
pub(super) async fn forward_progress(
    mut core_rx: broadcast::Receiver<ProtonInstallProgress>,
    out: ProgressEmitter,
) -> bool {
    let mut terminal = false;
    while let Some(progress) = next_progress(&mut core_rx).await {
        terminal |= matches!(
            progress.phase,
            Phase::Done | Phase::Failed | Phase::Cancelled
        );
        let message = messages::progress_message(&progress.phase, progress.message.as_deref());
        out.emit(
            progress.phase,
            progress.bytes_done,
            progress.bytes_total,
            message,
        );
    }
    terminal
}
