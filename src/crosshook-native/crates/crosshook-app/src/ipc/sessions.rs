//! `launch-session-changed` payloads and the `list_running_sessions` snapshot
//! DTO.
//!
//! The bus delivers `SessionStarted` / `SessionExited` in registry `seq`
//! order; the UI subscribes first, then fetches the snapshot, and applies only
//! events with `seq > snapshot.seq`. A bus lag becomes `{"kind":"resync"}`, on
//! which the UI re-fetches the snapshot and replaces its state. Payloads carry
//! no paths, PIDs, app ids or executable names.

use crosshook_core::launch::SessionKind;
use serde::Serialize;

use super::WireEvent;
use crate::services::launch::{RunningSession, RunningSessions};
use crate::AppEvent;

/// Event name for running-session changes.
pub const LAUNCH_SESSION_CHANGED_EVENT: &str = "launch-session-changed";

/// Wire session kind: `"game"` / `"trainer"`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKindDto {
    /// A game session.
    Game,
    /// A trainer session.
    Trainer,
}

impl From<SessionKind> for SessionKindDto {
    fn from(value: SessionKind) -> Self {
        match value {
            SessionKind::Game => Self::Game,
            SessionKind::Trainer => Self::Trainer,
        }
    }
}

/// `launch-session-changed` payload, tagged by `kind`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
// Fields mirror `AppEvent::SessionStarted`/`SessionExited`, which carry the docs.
#[allow(missing_docs)]
pub enum LaunchSessionChanged {
    Started {
        session_id: String,
        profile_name: Option<String>,
        session_kind: SessionKindDto,
        seq: u64,
    },
    Exited {
        session_id: String,
        profile_name: Option<String>,
        session_kind: SessionKindDto,
        code: Option<i32>,
        signal: Option<i32>,
        seq: u64,
    },
    Resync,
}

impl LaunchSessionChanged {
    /// The payload for a bus session event, or `None` for any other event.
    pub fn from_event(event: &AppEvent) -> Option<Self> {
        match event {
            AppEvent::SessionStarted {
                session_id,
                profile_name,
                kind,
                seq,
            } => Some(Self::Started {
                session_id: session_id.to_string(),
                profile_name: profile_name.clone(),
                session_kind: (*kind).into(),
                seq: *seq,
            }),
            AppEvent::SessionExited {
                session_id,
                profile_name,
                kind,
                code,
                signal,
                seq,
            } => Some(Self::Exited {
                session_id: session_id.to_string(),
                profile_name: profile_name.clone(),
                session_kind: (*kind).into(),
                code: *code,
                signal: *signal,
                seq: *seq,
            }),
            AppEvent::Resync { .. } => Some(Self::Resync),
            _ => None,
        }
    }
}

/// The `launch-session-changed` wire event `event` produces, if any.
pub fn session_changed_event(event: &AppEvent) -> Option<WireEvent> {
    let payload = LaunchSessionChanged::from_event(event)?;
    WireEvent::new(LAUNCH_SESSION_CHANGED_EVENT, &payload)
}

/// One entry of the `list_running_sessions` result.
#[derive(Debug, Clone, Serialize)]
pub struct RunningSessionDto {
    session_id: String,
    profile_name: Option<String>,
    session_kind: SessionKindDto,
}

/// `list_running_sessions` result: `{ sessions, seq }`.
#[derive(Debug, Clone, Serialize)]
pub struct RunningSessionsDto {
    sessions: Vec<RunningSessionDto>,
    seq: u64,
}

impl From<RunningSessions> for RunningSessionsDto {
    fn from(value: RunningSessions) -> Self {
        Self {
            sessions: value
                .sessions
                .into_iter()
                .map(
                    |RunningSession {
                         session_id,
                         profile_name,
                         kind,
                     }| RunningSessionDto {
                        session_id: session_id.to_string(),
                        profile_name,
                        session_kind: kind.into(),
                    },
                )
                .collect(),
            seq: value.seq,
        }
    }
}

#[cfg(test)]
mod tests;
