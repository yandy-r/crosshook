//! Conversions from `crosshook-app` / `crosshook-core` types (`app` feature).

use crosshook_app::services::launch::{
    RunningSession as AppRunningSession, RunningSessions as AppRunningSessions,
};
use crosshook_app::services::profiles::ProfileSummary;
use crosshook_app::AppEvent;
use crosshook_core::launch::{SessionId as CoreSessionId, SessionKind as CoreSessionKind};

use super::{ProfileCard, RunningSession, SessionEvent, SessionId, SessionKind};

impl ProfileCard {
    /// Builds a card from a profile summary. The favorite flag comes from the
    /// favorites list, which the summary does not carry.
    #[must_use]
    pub fn from_summary(summary: &ProfileSummary, is_favorite: bool) -> Self {
        Self {
            name: summary.name.clone(),
            game_name: summary.game_name.clone(),
            steam_app_id: summary.steam_app_id.clone(),
            is_favorite,
        }
    }
}

impl From<CoreSessionId> for SessionId {
    fn from(id: CoreSessionId) -> Self {
        Self::new(id.to_string())
    }
}

impl From<CoreSessionKind> for SessionKind {
    fn from(kind: CoreSessionKind) -> Self {
        match kind {
            CoreSessionKind::Game => Self::Game,
            CoreSessionKind::Trainer => Self::Trainer,
        }
    }
}

impl From<&AppRunningSession> for RunningSession {
    fn from(session: &AppRunningSession) -> Self {
        Self {
            session_id: session.session_id.into(),
            profile_name: session.profile_name.clone(),
            kind: session.kind.into(),
        }
    }
}

/// Converts a running-sessions snapshot into the sessions and `seq` of
/// [`LibraryIntent::RunningSnapshot`](super::LibraryIntent::RunningSnapshot).
/// The bridge adds the `token` from the
/// [`LibraryEffect::RequestRunningSessions`](super::LibraryEffect::RequestRunningSessions)
/// it answers.
#[must_use]
pub fn running_snapshot(snapshot: &AppRunningSessions) -> (Vec<RunningSession>, u64) {
    (
        snapshot.sessions.iter().map(RunningSession::from).collect(),
        snapshot.seq,
    )
}

/// The library's view of an app event: session start/exit and `Resync`.
/// Every other event (`AppEvent` is `#[non_exhaustive]`) is `None`.
#[must_use]
pub fn session_event(event: &AppEvent) -> Option<SessionEvent> {
    match event {
        AppEvent::Resync { .. } => Some(SessionEvent::Resync),
        AppEvent::SessionStarted {
            session_id,
            profile_name,
            kind,
            seq,
        } => Some(SessionEvent::Started {
            session_id: (*session_id).into(),
            profile_name: profile_name.clone(),
            kind: (*kind).into(),
            seq: *seq,
        }),
        AppEvent::SessionExited {
            session_id, seq, ..
        } => Some(SessionEvent::Exited {
            session_id: (*session_id).into(),
            seq: *seq,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
