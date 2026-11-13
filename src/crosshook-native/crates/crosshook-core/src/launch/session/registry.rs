//! Launch-session registry.
//!
//! Tracks active launch sessions (game + trainer) and lets the game session
//! broadcast a teardown signal to its linked trainer when it exits. Built so
//! trainer cleanup stays scoped to the trainer's own process tree — the
//! registry never inspects, kills, or reaches into another session's PIDs.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use tokio::sync::broadcast;

use super::types::{
    LinkError, SessionChange, SessionEntry, SessionId, SessionInfo, SessionKind, SessionObserver,
    TeardownReason,
};

/// In-memory registry of active launch sessions. Safe to share across tasks:
/// all mutation happens under a short-lived `Mutex` lock, and teardown
/// broadcasts are sent with the lock released so watchdog receivers do not
/// block the registry.
///
/// **Change sequence**: every insert and every actual removal bumps a
/// monotonic `seq` in the same critical section as the mutation, then calls
/// the optional [`SessionObserver`] while the lock is still held. Observers
/// therefore see changes in strict `seq` order, and
/// [`Self::snapshot`] returns a `(sessions, seq)` pair that is consistent
/// with that stream: a change with `seq <= snapshot_seq` is already in the
/// snapshot, a change with `seq > snapshot_seq` is not. Observers must not
/// block and must never call back into the registry.
///
/// **Poison policy**: every method calls `.expect("launch session registry
/// poisoned")` when locking. The registry has no recoverable degraded state
/// — if a thread panics while holding the lock (which requires a panic
/// during `HashMap` / `Vec` ops, typically OOM), every subsequent call
/// propagates a secondary panic rather than silently corrupting session
/// state. Treat poison as an unrecoverable invariant violation.
#[derive(Default)]
pub struct LaunchSessionRegistry {
    inner: Mutex<State>,
    observer: Option<SessionObserver>,
}

#[derive(Default)]
struct State {
    sessions: HashMap<SessionId, SessionEntry>,
    seq: u64,
}

impl LaunchSessionRegistry {
    /// Observer-less registry. Membership changes still bump `seq`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry that reports every membership change to `observer` (see the
    /// type-level docs for the ordering and re-entrancy contract).
    pub fn with_observer(observer: SessionObserver) -> Self {
        Self {
            inner: Mutex::default(),
            observer: Some(observer),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner.lock().expect("launch session registry poisoned")
    }

    /// Bump `seq` and notify the observer. Must be called with `state`
    /// still locked, right after the mutation it describes.
    fn record(&self, state: &mut State, change: impl FnOnce(u64) -> SessionChange) {
        state.seq += 1;
        let change = change(state.seq);
        if let Some(observer) = &self.observer {
            observer(&change);
        }
    }

    /// Register a new launch session. Returns the session id plus a receiver
    /// that the caller hands to its watchdog — when the registry later fires
    /// `cancel_linked_children` for the parent, this receiver will receive
    /// the [`TeardownReason`].
    pub fn register(
        &self,
        kind: SessionKind,
        profile_key: impl Into<String>,
    ) -> (SessionId, broadcast::Receiver<TeardownReason>) {
        let (entry, rx) = SessionEntry::new(kind, profile_key.into());
        let id = entry.id;
        let info = entry.info();
        let mut state = self.lock();
        state.sessions.insert(id, entry);
        self.record(&mut state, |seq| SessionChange::Started { info, seq });
        (id, rx)
    }

    /// Remove a session without an exit status. Idempotent — see
    /// [`Self::deregister_with_exit`].
    pub fn deregister(&self, id: SessionId) {
        self.deregister_with_exit(id, None, None);
    }

    /// Remove a session, recording the child's exit `code` / `signal` when
    /// known. Returns `true` — and emits exactly one
    /// [`SessionChange::Exited`] — only when the entry was actually removed;
    /// removing an unknown or already-removed id is a silent no-op, so the
    /// stream finalizer, its `Drop` fallback and the spawn-failure path can
    /// all call it safely.
    pub fn deregister_with_exit(
        &self,
        id: SessionId,
        code: Option<i32>,
        signal: Option<i32>,
    ) -> bool {
        let mut state = self.lock();
        let Some(entry) = state.sessions.remove(&id) else {
            return false;
        };
        let info = entry.info();
        self.record(&mut state, |seq| SessionChange::Exited {
            info,
            code,
            signal,
            seq,
        });
        true
    }

    /// Every active session plus the `seq` of the last change they reflect,
    /// read under one lock acquisition. Sessions are ordered oldest first.
    pub fn snapshot(&self) -> (Vec<SessionInfo>, u64) {
        let state = self.lock();
        let mut entries: Vec<&SessionEntry> = state.sessions.values().collect();
        entries.sort_by_key(|entry| entry.registered_at);
        (
            entries.into_iter().map(SessionEntry::info).collect(),
            state.seq,
        )
    }

    /// Attach a trainer session to its parent game session so a later
    /// `cancel_linked_children(game_id, …)` call reaches it. Returns a
    /// [`LinkError`] if the link would be invalid — trainer linking to
    /// another trainer, mismatched profile keys, missing ids, or double-link.
    pub fn link_to_parent(
        &self,
        child_id: SessionId,
        parent_id: SessionId,
    ) -> Result<(), LinkError> {
        let mut state = self.lock();
        let guard = &mut state.sessions;

        let (parent_kind, parent_profile) = {
            let parent = guard.get(&parent_id).ok_or(LinkError::ParentNotFound)?;
            (parent.kind, parent.profile_key.clone())
        };

        let child = guard.get_mut(&child_id).ok_or(LinkError::ChildNotFound)?;
        if child.parent.is_some() {
            return Err(LinkError::AlreadyLinked);
        }
        if child.kind != SessionKind::Trainer
            || parent_kind != SessionKind::Game
            || child.profile_key != parent_profile
        {
            return Err(LinkError::Incompatible);
        }
        child.parent = Some(parent_id);
        Ok(())
    }

    /// List session ids for a profile, optionally filtered by kind. Results
    /// are ordered **most-recently-registered first** so callers can pick
    /// the newest game as parent via `.into_iter().next()` deterministically,
    /// independent of the underlying `HashMap` iteration order.
    pub fn sessions_for_profile(
        &self,
        profile_key: &str,
        kind_filter: Option<SessionKind>,
    ) -> Vec<SessionId> {
        let state = self.lock();
        let guard = &state.sessions;
        let mut matches: Vec<&SessionEntry> = guard
            .values()
            .filter(|entry| entry.profile_key == profile_key)
            .filter(|entry| kind_filter.is_none_or(|kind| entry.kind == kind))
            .collect();
        // Most-recent first: sort by negation (reverse-sort of registered_at).
        matches.sort_by_key(|entry| std::cmp::Reverse(entry.registered_at));
        matches.into_iter().map(|entry| entry.id).collect()
    }

    /// Atomically register a new session and, if a compatible parent exists
    /// for the same profile with the given kind, link to it — all under a
    /// single lock acquisition. Closes the register → lookup → link race
    /// window where a parent could finalize between the trainer's
    /// `register` and its follow-up `link_to_parent`.
    ///
    /// Returns `(session_id, cancel_rx, Option<parent_session_id>)`. When no
    /// compatible parent is found (or when the kinds don't form a legal
    /// trainer → game link), the session is registered un-linked and the
    /// caller receives `None`. Parent selection favors the most
    /// recently-registered match.
    ///
    /// The `parent_kind` is the kind the parent must have. Currently only
    /// the trainer → game pairing is linked; any other combination registers
    /// the session without attempting a link (consistent with the validation
    /// in [`link_to_parent`]).
    pub fn register_and_link_to_parent_of_kind(
        &self,
        kind: SessionKind,
        profile_key: impl Into<String>,
        parent_kind: SessionKind,
    ) -> (
        SessionId,
        broadcast::Receiver<TeardownReason>,
        Option<SessionId>,
    ) {
        let profile_key = profile_key.into();
        let (mut entry, rx) = SessionEntry::new(kind, profile_key.clone());
        let child_id = entry.id;
        let mut state = self.lock();
        let guard = &mut state.sessions;

        // Only allow trainer → game links, mirroring link_to_parent's validation.
        let link_is_legal = kind == SessionKind::Trainer && parent_kind == SessionKind::Game;

        let parent_id = if link_is_legal {
            // Find the most-recently-registered parent candidate inside the
            // same lock acquisition used to insert the child.
            guard
                .values()
                .filter(|candidate| {
                    candidate.profile_key == profile_key && candidate.kind == parent_kind
                })
                .max_by_key(|candidate| candidate.registered_at)
                .map(|candidate| candidate.id)
        } else {
            None
        };

        if let Some(parent) = parent_id {
            entry.parent = Some(parent);
        }

        let info = entry.info();
        guard.insert(child_id, entry);
        self.record(&mut state, |seq| SessionChange::Started { info, seq });
        (child_id, rx, parent_id)
    }

    /// Broadcast `reason` to every child linked to `parent_id`. Returns the
    /// number of children that received the signal (even if their watchdog
    /// had no subscribers, the send is recorded).
    ///
    /// The lock is released before the actual `send` calls so a slow receiver
    /// cannot block registry mutation.
    pub fn cancel_linked_children(&self, parent_id: SessionId, reason: TeardownReason) -> usize {
        let senders: Vec<broadcast::Sender<TeardownReason>> = {
            let state = self.lock();
            let guard = &state.sessions;
            guard
                .values()
                .filter(|entry| entry.parent == Some(parent_id))
                .map(|entry| entry.cancel_tx.clone())
                .collect()
        };

        let mut signalled = 0usize;
        for sender in &senders {
            // send() on a channel with no live receivers returns Err but the
            // signal is still recorded for any future subscribers — we treat
            // either outcome as "delivered" for cleanup bookkeeping.
            let _ = sender.send(reason);
            signalled += 1;
        }
        signalled
    }

    /// Direct cancel for a single session — used by user-initiated teardown
    /// paths. Returns `true` if the session was registered.
    pub fn cancel_session(&self, id: SessionId, reason: TeardownReason) -> bool {
        let sender = {
            let state = self.lock();
            let guard = &state.sessions;
            guard.get(&id).map(|entry| entry.cancel_tx.clone())
        };
        if let Some(sender) = sender {
            let _ = sender.send(reason);
            true
        } else {
            false
        }
    }

    /// Cancel every active session registered under `profile_key`, across
    /// all [`SessionKind`]s — the user-initiated "reset" path. Linked
    /// children always share their parent's profile key (enforced by both
    /// [`Self::link_to_parent`] and the production trainer-spawn path
    /// [`Self::register_and_link_to_parent_of_kind`], which applies the same
    /// profile-key/kind validation atomically), so cancelling each matching
    /// session directly reaches them exactly once; running the
    /// [`Self::cancel_linked_children`] cascade on top would double-signal
    /// linked trainers. Returns the number of sessions signalled.
    pub fn cancel_sessions_for_profile(&self, profile_key: &str, reason: TeardownReason) -> usize {
        self.sessions_for_profile(profile_key, None)
            .into_iter()
            .filter(|id| self.cancel_session(*id, reason))
            .count()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.lock().sessions.len()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
