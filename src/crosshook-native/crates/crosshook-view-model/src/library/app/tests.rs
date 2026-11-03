use crosshook_app::services::launch::{RunningSession as AppSession, RunningSessions};
use crosshook_app::services::profiles::ProfileSummary;
use crosshook_app::{AppEvent, StartupJobKind};
use crosshook_core::launch::{LaunchSessionRegistry, SessionKind as CoreKind};

use super::{running_snapshot, session_event};
use crate::library::{ProfileCard, RunningSession, SessionEvent, SessionId, SessionKind};

fn core_session_id() -> crosshook_core::launch::SessionId {
    LaunchSessionRegistry::new()
        .register(CoreKind::Game, "alpha")
        .0
}

#[test]
fn summary_maps_card_fields_and_takes_favorite_separately() {
    let summary = ProfileSummary {
        name: "alpha".into(),
        game_name: "Alpha Game".into(),
        steam_app_id: "123".into(),
        custom_cover_art_path: Some("/art.png".into()),
        custom_portrait_art_path: None,
        network_isolation: true,
    };
    assert_eq!(
        ProfileCard::from_summary(&summary, true),
        ProfileCard {
            name: "alpha".into(),
            game_name: "Alpha Game".into(),
            steam_app_id: "123".into(),
            is_favorite: true,
        }
    );
}

#[test]
fn snapshot_keeps_ids_by_display_kinds_and_seq() {
    let (game, trainer) = (core_session_id(), core_session_id());
    let snapshot = RunningSessions {
        sessions: vec![
            AppSession {
                session_id: game,
                profile_name: Some("alpha".into()),
                kind: CoreKind::Game,
            },
            AppSession {
                session_id: trainer,
                profile_name: None,
                kind: CoreKind::Trainer,
            },
        ],
        seq: 7,
    };
    assert_eq!(
        running_snapshot(&snapshot),
        (
            vec![
                RunningSession {
                    session_id: SessionId::new(game.to_string()),
                    profile_name: Some("alpha".into()),
                    kind: SessionKind::Game,
                },
                RunningSession {
                    session_id: SessionId::new(trainer.to_string()),
                    profile_name: None,
                    kind: SessionKind::Trainer,
                },
            ],
            7
        )
    );
}

#[test]
fn session_events_map_and_other_events_are_ignored() {
    let id = core_session_id();
    assert_eq!(
        session_event(&AppEvent::SessionStarted {
            session_id: id,
            profile_name: Some("alpha".into()),
            kind: CoreKind::Trainer,
            seq: 3,
        }),
        Some(SessionEvent::Started {
            session_id: SessionId::new(id.to_string()),
            profile_name: Some("alpha".into()),
            kind: SessionKind::Trainer,
            seq: 3,
        })
    );
    assert_eq!(
        session_event(&AppEvent::SessionExited {
            session_id: id,
            profile_name: Some("alpha".into()),
            kind: CoreKind::Trainer,
            code: Some(0),
            signal: None,
            seq: 4,
        }),
        Some(SessionEvent::Exited {
            session_id: SessionId::new(id.to_string()),
            seq: 4,
        })
    );
    assert_eq!(
        session_event(&AppEvent::Resync { missed: 2 }),
        Some(SessionEvent::Resync)
    );
    assert_eq!(
        session_event(&AppEvent::StartupJobFinished {
            job: StartupJobKind::HealthBatch,
        }),
        None
    );
}
