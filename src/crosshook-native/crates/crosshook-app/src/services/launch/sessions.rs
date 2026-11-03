//! Running-session events and snapshot (YAN-824).
//!
//! The registry reports every membership change to an observer while its lock
//! is held; [`session_observer`] turns each change into an
//! [`AppEvent::SessionStarted`] / [`AppEvent::SessionExited`] on the app bus,
//! so bus order equals registry `seq` order. [`LaunchService::running_sessions_snapshot`]
//! reads the sessions and the same `seq` under one lock, so a subscriber that
//! subscribes first, then snapshots, applies exactly the events with
//! `seq > snapshot.seq`.

use std::sync::Arc;

use crosshook_core::launch::{
    SessionChange, SessionId, SessionInfo, SessionKind, SessionObserver, ANONYMOUS_PROFILE_KEY,
};

use super::LaunchService;
use crate::events::{AppEvent, EventBus};

/// One registered launch session, as presenters see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningSession {
    /// Registry id of the session.
    pub session_id: SessionId,
    /// Trimmed profile name; `None` for unsaved (anonymous) launches.
    pub profile_name: Option<String>,
    /// Game or trainer.
    pub kind: SessionKind,
}

/// Every registered session plus the registry `seq` they reflect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningSessions {
    /// Sessions, oldest first.
    pub sessions: Vec<RunningSession>,
    /// Sequence of the last change the list reflects.
    pub seq: u64,
}

impl LaunchService {
    /// Registered sessions (game and trainer) and the change `seq` they
    /// reflect, read under one registry lock.
    #[must_use]
    pub fn running_sessions_snapshot(&self) -> RunningSessions {
        let (sessions, seq) = self.registry.snapshot();
        RunningSessions {
            sessions: sessions.iter().map(running_session).collect(),
            seq,
        }
    }
}

/// Registry observer publishing each change to `events`. `EventBus::publish`
/// never blocks and never touches the registry, which is the observer
/// contract.
pub(super) fn session_observer(events: EventBus) -> SessionObserver {
    Arc::new(move |change| {
        events.publish(session_event(change));
    })
}

fn session_event(change: &SessionChange) -> AppEvent {
    match change {
        SessionChange::Started { info, seq } => AppEvent::SessionStarted {
            session_id: info.session_id,
            profile_name: profile_name(&info.profile_key),
            kind: info.kind,
            seq: *seq,
        },
        SessionChange::Exited {
            info,
            code,
            signal,
            seq,
        } => AppEvent::SessionExited {
            session_id: info.session_id,
            profile_name: profile_name(&info.profile_key),
            kind: info.kind,
            code: *code,
            signal: *signal,
            seq: *seq,
        },
    }
}

fn running_session(info: &SessionInfo) -> RunningSession {
    RunningSession {
        session_id: info.session_id,
        profile_name: profile_name(&info.profile_key),
        kind: info.kind,
    }
}

/// The registry key is the trimmed profile name, or the anonymous sentinel
/// (which profile validation reserves) for unsaved launches.
fn profile_name(profile_key: &str) -> Option<String> {
    (profile_key != ANONYMOUS_PROFILE_KEY).then(|| profile_key.to_string())
}
