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

    /// List session ids for a profile, optionally filtered by kind. Used by
    /// the trainer spawn path to discover its parent game session.
    pub fn sessions_for_profile(
        &self,
        profile_key: &str,
        kind_filter: Option<SessionKind>,
    ) -> Vec<SessionId> {
        let guard = self.inner.lock().expect("launch session registry poisoned");
        guard
            .values()
            .filter(|entry| entry.profile_key == profile_key)
            .filter(|entry| kind_filter.is_none_or(|kind| entry.kind == kind))
            .map(|entry| entry.id)
            .collect()
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

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("launch session registry poisoned")
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn registry_is_send_and_sync() {
        assert_send_sync::<LaunchSessionRegistry>();
    }

    #[test]
    fn register_and_deregister_round_trip() {
        let registry = LaunchSessionRegistry::new();
        let (id, _rx) = registry.register(SessionKind::Game, "profile-a");
        assert_eq!(registry.len(), 1);

        registry.deregister(id);
        assert_eq!(registry.len(), 0);

        // Idempotent.
        registry.deregister(id);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn link_to_parent_happy_path() {
        let registry = LaunchSessionRegistry::new();
        let (game_id, _game_rx) = registry.register(SessionKind::Game, "profile-a");
        let (trainer_id, _trainer_rx) = registry.register(SessionKind::Trainer, "profile-a");

        registry
            .link_to_parent(trainer_id, game_id)
            .expect("link should succeed");
    }

    #[test]
    fn link_to_parent_rejects_missing_parent() {
        let registry = LaunchSessionRegistry::new();
        let (trainer_id, _rx) = registry.register(SessionKind::Trainer, "profile-a");
        let phantom = SessionId::new();

        assert_eq!(
            registry.link_to_parent(trainer_id, phantom),
            Err(LinkError::ParentNotFound),
        );
    }

    #[test]
    fn link_to_parent_rejects_missing_child() {
        let registry = LaunchSessionRegistry::new();
        let (game_id, _rx) = registry.register(SessionKind::Game, "profile-a");
        let phantom = SessionId::new();

        assert_eq!(
            registry.link_to_parent(phantom, game_id),
            Err(LinkError::ChildNotFound),
        );
    }

    #[test]
    fn link_to_parent_rejects_cross_profile() {
        let registry = LaunchSessionRegistry::new();
        let (game_id, _g) = registry.register(SessionKind::Game, "profile-a");
        let (trainer_id, _t) = registry.register(SessionKind::Trainer, "profile-b");

        assert_eq!(
            registry.link_to_parent(trainer_id, game_id),
            Err(LinkError::Incompatible),
        );
    }

    #[test]
    fn link_to_parent_rejects_trainer_as_parent() {
        let registry = LaunchSessionRegistry::new();
        let (p, _p_rx) = registry.register(SessionKind::Trainer, "profile-a");
        let (c, _c_rx) = registry.register(SessionKind::Trainer, "profile-a");

        assert_eq!(registry.link_to_parent(c, p), Err(LinkError::Incompatible),);
    }

    #[test]
    fn link_to_parent_rejects_double_link() {
        let registry = LaunchSessionRegistry::new();
        let (g1, _g1_rx) = registry.register(SessionKind::Game, "profile-a");
        let (g2, _g2_rx) = registry.register(SessionKind::Game, "profile-a");
        let (t, _t_rx) = registry.register(SessionKind::Trainer, "profile-a");

        registry.link_to_parent(t, g1).expect("first link ok");
        assert_eq!(
            registry.link_to_parent(t, g2),
            Err(LinkError::AlreadyLinked),
        );
    }

    #[tokio::test]
    async fn cancel_linked_children_reaches_only_linked_children() {
        let registry = LaunchSessionRegistry::new();
        let (game_id, _game_rx) = registry.register(SessionKind::Game, "profile-a");
        let (linked_trainer_id, mut linked_rx) =
            registry.register(SessionKind::Trainer, "profile-a");
        let (unlinked_trainer_id, mut unlinked_rx) =
            registry.register(SessionKind::Trainer, "profile-a");

        registry
            .link_to_parent(linked_trainer_id, game_id)
            .expect("link ok");

        let signalled = registry.cancel_linked_children(game_id, TeardownReason::LinkedSessionExit);
        assert_eq!(signalled, 1, "only the linked trainer should be signalled");

        let received = linked_rx.recv().await.expect("linked trainer gets signal");
        assert_eq!(received, TeardownReason::LinkedSessionExit);
        assert!(
            unlinked_rx.try_recv().is_err(),
            "unlinked trainer must not receive the cancel"
        );

        // Sanity — silence unused warnings.
        let _ = unlinked_trainer_id;
    }

    #[tokio::test]
    async fn cancel_session_targets_exactly_one() {
        let registry = LaunchSessionRegistry::new();
        let (id, mut rx) = registry.register(SessionKind::Trainer, "profile-a");

        assert!(registry.cancel_session(id, TeardownReason::UserRequest));
        let received = rx.recv().await.expect("session receives cancel");
        assert_eq!(received, TeardownReason::UserRequest);

        registry.deregister(id);
        assert!(!registry.cancel_session(id, TeardownReason::UserRequest));
    }

    #[test]
    fn sessions_for_profile_filters_by_kind() {
        let registry = LaunchSessionRegistry::new();
        let (game_id, _g_rx) = registry.register(SessionKind::Game, "profile-a");
        let (trainer_id, _t_rx) = registry.register(SessionKind::Trainer, "profile-a");
        let (_other_game_id, _o_rx) = registry.register(SessionKind::Game, "profile-b");

        let games = registry.sessions_for_profile("profile-a", Some(SessionKind::Game));
        assert_eq!(games, vec![game_id]);

        let trainers = registry.sessions_for_profile("profile-a", Some(SessionKind::Trainer));
        assert_eq!(trainers, vec![trainer_id]);

        let all = registry.sessions_for_profile("profile-a", None);
        assert_eq!(all.len(), 2);
    }
}
