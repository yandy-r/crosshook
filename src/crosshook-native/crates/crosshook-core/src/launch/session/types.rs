//! Launch-session data types.
//!
//! A [`LaunchSession`] tracks the lifecycle of one running launch (game or
//! trainer) so the registry can broadcast teardown signals between linked
//! sessions without touching each other's process trees.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Broadcast channel capacity. A session never has more than one trainer
/// child linked to it at a time, so capacity 4 leaves headroom for repeated
/// cancels (idempotent) without lagging the receiver.
pub(crate) const SESSION_CANCEL_CHANNEL_CAPACITY: usize = 4;

/// Opaque identifier for a launch session.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Classifies a launch session so the registry can filter parent candidates
/// (trainers link to the game in the same profile).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SessionKind {
    Game,
    Trainer,
}

/// Why a session was torn down. Persisted into `launch_operations.diagnostic_json`
/// so operators can trace cleanup paths after the fact.
///
/// Serialized as snake_case to match the surrounding diagnostics schema
/// (`FailureMode`, `ValidationSeverity`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeardownReason {
    /// Process exited on its own; no watchdog intervention needed.
    NaturalExit,
    /// Watchdog observed the target game exe disappear and terminated gamescope.
    WatchdogNaturalExit,
    /// Parent session ended and broadcast a cancel to this child.
    LinkedSessionExit,
    /// User explicitly requested teardown via UI or CLI.
    UserRequest,
    /// Cancel channel closed before any signal arrived — the registry entry
    /// was torn down from underneath the watchdog (typically because the
    /// child process exited first and the stream finalizer deregistered
    /// before the watchdog's `recv()` had a chance to return `Ok`). Kept
    /// distinct from [`LinkedSessionExit`] so the diagnostic audit trail
    /// does not falsely attribute a teardown to a parent-cascade signal.
    ReceiverClosed,
}

impl TeardownReason {
    /// Short machine-readable identifier used in structured logs.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NaturalExit => "natural_exit",
            Self::WatchdogNaturalExit => "watchdog_natural_exit",
            Self::LinkedSessionExit => "linked_session_exit",
            Self::UserRequest => "user_request",
            Self::ReceiverClosed => "receiver_closed",
        }
    }
}

impl fmt::Display for TeardownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Internal registry entry. The `cancel_tx` lets the registry signal a running
/// watchdog that its owning launch should tear down (e.g. the parent game
/// exited). `parent` is set by [`link_to_parent`][super::registry::LaunchSessionRegistry::link_to_parent]
/// so `cancel_linked_children` can find child sessions.
///
/// `registered_at` records when the entry was inserted so
/// [`sessions_for_profile`][super::registry::LaunchSessionRegistry::sessions_for_profile]
/// can return deterministic, insertion-ordered results regardless of the
/// underlying `HashMap` iteration order — used by the trainer spawn path to
/// prefer the most-recently-registered game as its parent.
#[derive(Clone)]
pub(super) struct SessionEntry {
    pub(super) id: SessionId,
    pub(super) kind: SessionKind,
    pub(super) profile_key: String,
    pub(super) parent: Option<SessionId>,
    pub(super) cancel_tx: broadcast::Sender<TeardownReason>,
    pub(super) registered_at: Instant,
}

impl SessionEntry {
    pub(super) fn new(
        kind: SessionKind,
        profile_key: String,
    ) -> (Self, broadcast::Receiver<TeardownReason>) {
        let (cancel_tx, cancel_rx) = broadcast::channel(SESSION_CANCEL_CHANNEL_CAPACITY);
        let entry = Self {
            id: SessionId::new(),
            kind,
            profile_key,
            parent: None,
            cancel_tx,
            registered_at: Instant::now(),
        };
        (entry, cancel_rx)
    }
}

/// Error cases for linking a trainer session to its parent game session.
#[derive(Debug, Eq, PartialEq)]
pub enum LinkError {
    /// Child session is not in the registry.
    ChildNotFound,
    /// Parent session is not in the registry (e.g. already deregistered).
    ParentNotFound,
    /// Linking would cross profiles or session kinds (e.g. trainer → trainer).
    Incompatible,
    /// Child is already linked to a parent.
    AlreadyLinked,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChildNotFound => f.write_str("child session not found in registry"),
            Self::ParentNotFound => f.write_str("parent session not found in registry"),
            Self::Incompatible => {
                f.write_str("parent/child session kinds or profiles incompatible")
            }
            Self::AlreadyLinked => f.write_str("child session already linked to a parent"),
        }
    }
}

impl std::error::Error for LinkError {}

/// Shared slot the gamescope watchdog writes to when it tears down a tree.
/// The stream finalizer reads this after the child exits so it can mark the
/// launch's `diagnostic_json` with the teardown reason.
///
/// Replaces the previous `Arc<AtomicBool>` "watchdog_killed" flag — this
/// carries the same "did the watchdog fire" boolean plus the semantic reason.
#[derive(Clone, Default)]
pub struct WatchdogOutcome {
    inner: Arc<Mutex<WatchdogOutcomeInner>>,
}

#[derive(Default)]
struct WatchdogOutcomeInner {
    killed: bool,
    reason: Option<TeardownReason>,
}

impl WatchdogOutcome {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that the watchdog tore down the gamescope tree. Idempotent —
    /// repeated calls keep the first recorded reason (including a reason
    /// already set by [`record_reason`]) so the finalizer gets a stable
    /// view. Always flips `killed` to true on the first invocation.
    pub fn mark(&self, reason: TeardownReason) {
        let mut guard = self.inner.lock().expect("watchdog outcome poisoned");
        if guard.reason.is_none() {
            guard.reason = Some(reason);
        }
        guard.killed = true;
    }

    /// Record a teardown reason *without* flagging a kill. Used by launch
    /// paths that observe a cancel signal but have no gamescope tree to tear
    /// down (e.g. a trainer without gamescope that receives a parent-cascade
    /// cancel). Lets the stream finalizer stamp the correct
    /// [`TeardownReason`] in diagnostics while leaving `was_killed()` false
    /// so it doesn't rewrite the summary as if a compositor had been cleaned
    /// up. Keeps the first recorded reason on repeated calls.
    pub fn record_reason(&self, reason: TeardownReason) {
        let mut guard = self.inner.lock().expect("watchdog outcome poisoned");
        if guard.reason.is_none() {
            guard.reason = Some(reason);
        }
    }

    pub fn was_killed(&self) -> bool {
        let guard = self.inner.lock().expect("watchdog outcome poisoned");
        guard.killed
    }

    pub fn reason(&self) -> Option<TeardownReason> {
        let guard = self.inner.lock().expect("watchdog outcome poisoned");
        guard.reason
    }
}

impl fmt::Debug for WatchdogOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let guard = self.inner.lock().expect("watchdog outcome poisoned");
        f.debug_struct("WatchdogOutcome")
            .field("killed", &guard.killed)
            .field("reason", &guard.reason)
            .finish()
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;

    #[test]
    fn outcome_default_is_not_killed() {
        let outcome = WatchdogOutcome::new();
        assert!(!outcome.was_killed());
        assert_eq!(outcome.reason(), None);
    }

    #[test]
    fn outcome_mark_sets_killed_and_reason() {
        let outcome = WatchdogOutcome::new();
        outcome.mark(TeardownReason::LinkedSessionExit);
        assert!(outcome.was_killed());
        assert_eq!(outcome.reason(), Some(TeardownReason::LinkedSessionExit));
    }

    #[test]
    fn outcome_mark_is_idempotent_first_reason_wins() {
        let outcome = WatchdogOutcome::new();
        outcome.mark(TeardownReason::WatchdogNaturalExit);
        outcome.mark(TeardownReason::LinkedSessionExit);
        assert_eq!(outcome.reason(), Some(TeardownReason::WatchdogNaturalExit));
    }

    #[test]
    fn outcome_clone_shares_state() {
        let a = WatchdogOutcome::new();
        let b = a.clone();
        a.mark(TeardownReason::UserRequest);
        assert!(b.was_killed());
        assert_eq!(b.reason(), Some(TeardownReason::UserRequest));
    }

    #[test]
    fn outcome_record_reason_does_not_flag_killed() {
        let outcome = WatchdogOutcome::new();
        outcome.record_reason(TeardownReason::LinkedSessionExit);
        assert!(!outcome.was_killed(), "record_reason must not set killed");
        assert_eq!(outcome.reason(), Some(TeardownReason::LinkedSessionExit));
    }

    #[test]
    fn outcome_record_reason_keeps_first_then_mark_sets_killed() {
        let outcome = WatchdogOutcome::new();
        outcome.record_reason(TeardownReason::LinkedSessionExit);
        // A later mark() should flip killed=true but keep the first reason.
        outcome.mark(TeardownReason::WatchdogNaturalExit);
        assert!(outcome.was_killed());
        assert_eq!(outcome.reason(), Some(TeardownReason::LinkedSessionExit));
    }
}
