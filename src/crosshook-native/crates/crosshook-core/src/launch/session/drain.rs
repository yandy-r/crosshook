//! Cancel-channel drain helper for launch sessions that have no gamescope
//! watchdog to consume their [`broadcast::Receiver`].
//!
//! Lives next to the registry rather than in the Tauri layer because the
//! mapping from a [`broadcast`] receive outcome to a [`TeardownReason`] is
//! core business logic — same crate as [`cancel_reason`] in
//! `launch/watchdog/tasks.rs`. Keeps cancel-channel semantics in one place.

use tokio::sync::broadcast;

use super::types::{SessionId, TeardownReason, WatchdogOutcome};
use crate::launch::watchdog::cancel_reason_after_lag;

/// Drain a session's cancel receiver and record the observed [`TeardownReason`]
/// into `outcome` via [`WatchdogOutcome::record_reason`] — without flagging
/// `was_killed`, because this path has no gamescope tree to tear down. The
/// stream finalizer reads `outcome.reason()` to stamp
/// `DiagnosticReport.teardown_reason` correctly even though no watchdog
/// fired.
///
/// Use this for any launch session where the watchdog-spawn guard (e.g.
/// gamescope disabled, or `child.id()` returned `None`) left `cancel_rx`
/// without a live consumer. Without this drain, cascades targeting the
/// session would be lost and `teardown_reason` would default to
/// `NaturalExit`, misleading the audit trail.
///
/// On `Closed` (registry entry deregistered from underneath) the function
/// intentionally does **not** record anything — the session is already
/// being finalized by its stream handler, so there's no cancel semantic to
/// attribute.
pub async fn drain_cancel_into_outcome(
    session_id: SessionId,
    outcome: WatchdogOutcome,
    mut cancel_rx: broadcast::Receiver<TeardownReason>,
) {
    match cancel_rx.recv().await {
        Ok(reason) => {
            outcome.record_reason(reason);
            tracing::info!(
                session_id = %session_id,
                teardown_reason = %reason,
                "launch session without watchdog received cancel; process exit will finalize the session"
            );
        }
        Err(broadcast::error::RecvError::Closed) => {
            tracing::debug!(
                session_id = %session_id,
                "launch session cancel channel closed before any signal"
            );
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let reason = cancel_reason_after_lag(&mut cancel_rx);
            outcome.record_reason(reason);
            tracing::debug!(
                session_id = %session_id,
                teardown_reason = %reason,
                "launch session cancel channel lagged; recovered reason via try_recv"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry::LaunchSessionRegistry;
    use super::super::types::SessionKind;
    use super::*;

    #[tokio::test]
    async fn drain_records_reason_on_cascade() {
        let registry = LaunchSessionRegistry::new();
        let (game_id, _g_rx) = registry.register(SessionKind::Game, "profile-a");
        let (trainer_id, trainer_rx, _parent) = registry.register_and_link_to_parent_of_kind(
            SessionKind::Trainer,
            "profile-a",
            SessionKind::Game,
        );
        let outcome = WatchdogOutcome::new();
        let drain = tokio::spawn(drain_cancel_into_outcome(
            trainer_id,
            outcome.clone(),
            trainer_rx,
        ));

        registry.cancel_linked_children(game_id, TeardownReason::LinkedSessionExit);
        drain.await.expect("drain should complete");

        assert!(
            !outcome.was_killed(),
            "drain must never set killed — there was no gamescope tree"
        );
        assert_eq!(outcome.reason(), Some(TeardownReason::LinkedSessionExit));
    }

    #[tokio::test]
    async fn drain_records_nothing_when_channel_closes_first() {
        let registry = LaunchSessionRegistry::new();
        let (trainer_id, trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");
        let outcome = WatchdogOutcome::new();

        // Deregister before any signal is sent so the sender drops and the
        // receiver sees Closed on first recv.
        registry.deregister(trainer_id);
        drain_cancel_into_outcome(trainer_id, outcome.clone(), trainer_rx).await;

        assert!(!outcome.was_killed());
        assert_eq!(
            outcome.reason(),
            None,
            "Closed is attributed at the finalizer level (NaturalExit fallback), \
             not by the drain helper"
        );
    }
}
