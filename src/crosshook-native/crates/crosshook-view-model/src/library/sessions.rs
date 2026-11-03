//! Running-session tracker behind the Currently Playing count and the
//! `CurrentlyRunning` filter.
//!
//! Follows the backend seq contract (`crosshook-app` `services/launch/sessions.rs`):
//! the bridge subscribes to session events first, then requests a snapshot;
//! events that arrive before the snapshot are buffered, and the snapshot
//! applies only the buffered events with `seq > snapshot.seq`.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::fmt;

/// Most session events buffered while no snapshot is applied. On overflow the
/// buffer is dropped and a new snapshot is requested.
pub(crate) const MAX_BUFFERED_EVENTS: usize = 1024;

/// Opaque launch-session id. With the `app` feature it is built from the
/// `Display` form of `crosshook_core::launch::SessionId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(String);

impl SessionId {
    /// Wraps a session id string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a launch session runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {
    /// The game process. Only game sessions count as "currently playing".
    Game,
    /// A trainer process.
    Trainer,
}

/// One live launch session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningSession {
    /// Registry id of the session.
    pub session_id: SessionId,
    /// Trimmed profile name; `None` for unsaved (anonymous) launches.
    pub profile_name: Option<String>,
    /// Game or trainer.
    pub kind: SessionKind,
}

/// A session change from the app event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// A session entered the registry.
    Started {
        /// Registry id of the session.
        session_id: SessionId,
        /// Trimmed profile name; `None` for unsaved (anonymous) launches.
        profile_name: Option<String>,
        /// Game or trainer.
        kind: SessionKind,
        /// Registry change sequence.
        seq: u64,
    },
    /// A session left the registry.
    Exited {
        /// Registry id of the session.
        session_id: SessionId,
        /// Registry change sequence.
        seq: u64,
    },
    /// The subscriber missed events; the state must be re-read from a snapshot.
    Resync,
}

impl SessionEvent {
    fn seq(&self) -> Option<u64> {
        match self {
            Self::Started { seq, .. } | Self::Exited { seq, .. } => Some(*seq),
            Self::Resync => None,
        }
    }
}

/// Live sessions and the set of running profile names derived from them.
///
/// Rules (React `runningSessionsStore.ts` parity):
/// - Before the first snapshot, and after a [`SessionEvent::Resync`], started
///   and exited events are buffered (at most 1024; on overflow the buffer is
///   dropped and [`apply`](Self::apply) asks for a new snapshot once).
/// - A snapshot replaces the state, then applies the buffered events with
///   `seq > snapshot.seq` in arrival order.
/// - Once synced, events with `seq <= snapshot.seq` are ignored; a duplicate
///   start and an exit for an unknown id are no-ops.
/// - A snapshot with a lower `seq` than a change the tracker already knows
///   about (an earlier snapshot, an applied event, or an event dropped on
///   overflow) is stale and ignored. A snapshot requested after an event is
///   never older than that event, so the latest request's answer always applies.
#[derive(Debug, Clone, Default)]
pub struct RunningProfiles {
    sessions: BTreeMap<SessionId, RunningSession>,
    /// Live named game sessions per profile name.
    game_names: BTreeMap<String, usize>,
    baseline: Option<u64>,
    /// Highest `seq` the state reflects or has dropped; snapshots with a
    /// lower `seq` are stale.
    stale_below: u64,
    buffer: Vec<SessionEvent>,
}

impl RunningProfiles {
    /// Replaces the state with `sessions` taken at `seq`, then applies the
    /// buffered events newer than `seq`. Ignored when the snapshot is stale.
    pub fn apply_snapshot(&mut self, sessions: Vec<RunningSession>, seq: u64) {
        if seq < self.stale_below {
            return;
        }
        self.sessions.clear();
        self.game_names.clear();
        for session in sessions {
            self.insert(session);
        }
        self.baseline = Some(seq);
        self.stale_below = seq;
        for event in std::mem::take(&mut self.buffer) {
            self.apply_live(event);
        }
    }

    /// Applies one event. Returns `true` when a new snapshot must be
    /// requested: on [`SessionEvent::Resync`] and when the pre-snapshot
    /// buffer overflows.
    pub fn apply(&mut self, event: SessionEvent) -> bool {
        if matches!(event, SessionEvent::Resync) {
            self.baseline = None;
            return true;
        }
        if self.baseline.is_some() {
            self.apply_live(event);
            return false;
        }
        if self.buffer.len() < MAX_BUFFERED_EVENTS {
            self.buffer.push(event);
            return false;
        }
        // The requested snapshot is taken after every dropped event, so it
        // covers them; any snapshot older than they are is stale.
        let dropped = self
            .buffer
            .iter()
            .filter_map(SessionEvent::seq)
            .chain(event.seq())
            .max()
            .unwrap_or(0);
        self.stale_below = self.stale_below.max(dropped);
        self.buffer.clear();
        true
    }

    /// Whether `profile_name` has a live game session (exact, case-sensitive).
    #[must_use]
    pub fn contains(&self, profile_name: &str) -> bool {
        self.game_names.contains_key(profile_name)
    }

    /// Number of distinct profile names with a live game session. Trainer and
    /// anonymous sessions do not count. Not intersected with the loaded
    /// profiles (a running profile that was renamed still counts).
    #[must_use]
    pub fn currently_playing_count(&self) -> usize {
        self.game_names.len()
    }

    /// Whether a snapshot is applied and no resync is pending.
    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.baseline.is_some()
    }

    fn apply_live(&mut self, event: SessionEvent) {
        let Some(seq) = event.seq() else {
            return;
        };
        if self.baseline.is_some_and(|baseline| seq <= baseline) {
            return;
        }
        self.stale_below = self.stale_below.max(seq);
        match event {
            SessionEvent::Started {
                session_id,
                profile_name,
                kind,
                ..
            } => self.insert(RunningSession {
                session_id,
                profile_name,
                kind,
            }),
            SessionEvent::Exited { session_id, .. } => self.remove(&session_id),
            SessionEvent::Resync => {}
        }
    }

    fn insert(&mut self, session: RunningSession) {
        let Entry::Vacant(slot) = self.sessions.entry(session.session_id.clone()) else {
            return;
        };
        if let (SessionKind::Game, Some(name)) = (session.kind, &session.profile_name) {
            *self.game_names.entry(name.clone()).or_default() += 1;
        }
        slot.insert(session);
    }

    fn remove(&mut self, session_id: &SessionId) {
        let Some(session) = self.sessions.remove(session_id) else {
            return;
        };
        if let (SessionKind::Game, Some(name)) = (session.kind, session.profile_name) {
            if let Entry::Occupied(mut count) = self.game_names.entry(name) {
                *count.get_mut() -= 1;
                if *count.get() == 0 {
                    count.remove();
                }
            }
        }
    }
}
