//! Launch-session registry.
//!
//! Tracks active launch sessions (game + trainer) and lets the game session
//! broadcast a teardown signal to its linked trainer when it exits. Built so
//! trainer cleanup stays scoped to the trainer's own process tree — the
//! registry never inspects, kills, or reaches into another session's PIDs.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::broadcast;

use super::types::{LinkError, SessionEntry, SessionId, SessionKind, TeardownReason};

/// In-memory registry of active launch sessions. Safe to share across tasks:
/// all mutation happens under a short-lived `Mutex` lock, and teardown
/// broadcasts are sent with the lock released so watchdog receivers do not
/// block the registry.
///
/// **Poison policy**: every method calls `.expect("launch session registry
/// poisoned")` when locking. The registry has no recoverable degraded state
/// — if a thread panics while holding the lock (which requires a panic
/// during `HashMap` / `Vec` ops, typically OOM), every subsequent call
/// propagates a secondary panic rather than silently corrupting session
/// state. Treat poison as an unrecoverable invariant violation.
#[derive(Default)]
pub struct LaunchSessionRegistry {
    inner: Mutex<HashMap<SessionId, SessionEntry>>,
}

impl LaunchSessionRegistry {
    pub fn new() -> Self {
        Self::default()
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
        let mut guard = self.inner.lock().expect("launch session registry poisoned");
        guard.insert(id, entry);
        (id, rx)
    }

    /// Remove a session. Idempotent — double-deregister is a no-op so the
    /// stream finalizer and watchdog can both safely call it.
    pub fn deregister(&self, id: SessionId) {
        let mut guard = self.inner.lock().expect("launch session registry poisoned");
        guard.remove(&id);
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
        let mut guard = self.inner.lock().expect("launch session registry poisoned");

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
        let guard = self.inner.lock().expect("launch session registry poisoned");
        let mut matches: Vec<&SessionEntry> = guard
            .values()
            .filter(|entry| entry.profile_key == profile_key)
            .filter(|entry| kind_filter.is_none_or(|kind| entry.kind == kind))
            .collect();
        // Most-recent first: sort by negation (reverse-sort of registered_at).
        matches.sort_by_key(|entry| std::cmp::Reverse(entry.registered_at));
        matches.into_iter().map(|entry| entry.id).collect()
    }

    /// List active profile keys, optionally filtered by session kind.
    /// Results are sorted and de-duplicated for deterministic read-only UI
    /// consumers.
    pub fn active_profile_keys(&self, kind_filter: Option<SessionKind>) -> Vec<String> {
        let mut profile_keys: Vec<String> = {
            let guard = self.inner.lock().expect("launch session registry poisoned");
            guard
                .values()
                .filter(|entry| kind_filter.is_none_or(|kind| entry.kind == kind))
                .map(|entry| entry.profile_key.clone())
                .collect()
        };
        profile_keys.sort();
        profile_keys.dedup();
        profile_keys
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
        let mut guard = self.inner.lock().expect("launch session registry poisoned");

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

        guard.insert(child_id, entry);
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
            let guard = self.inner.lock().expect("launch session registry poisoned");
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
            let guard = self.inner.lock().expect("launch session registry poisoned");
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
        self.inner
            .lock()
            .expect("launch session registry poisoned")
            .len()
    }
}

// Tests extracted to a sibling file to keep this production module under the
// repo's ~500-line soft cap — mirrors the `#[path]` pattern used by
// `profile::lutris_import::tests`.
#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
