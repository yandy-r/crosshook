//! Start-path contracts exercised through the public entry points.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crosshook_core::launch::{LaunchSessionRegistry, LinkError, SessionId, SessionKind};
use crosshook_core::profile::{HookStage, LaunchHook};
use tokio::process::{Child, Command};

use super::super::{InjectionLevel, LaunchEvent, ProcessSpawner};
use super::helpers::{FakeSpawner, Harness, Metadata, Script};
use crate::log_tail::LogBatch;

fn logs_for_slug(slug: &str) -> Vec<PathBuf> {
    let prefixes = [format!("game-{slug}-"), format!("trainer-{slug}-")];
    match fs::read_dir("/tmp/crosshook-logs") {
        Ok(entries) => entries
            .map(|entry| entry.expect("log directory entry").path())
            .filter(|path| {
                path.file_name().is_some_and(|name| {
                    prefixes
                        .iter()
                        .any(|prefix| name.to_string_lossy().starts_with(prefix))
                })
            })
            .collect(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("read log directory: {error}"),
    }
}

#[tokio::test]
async fn invalid_requests_fail_before_any_side_effect() {
    let harness = Harness::new(Metadata::Enabled);
    for trainer in [false, true] {
        let mut request = harness.proton_request("invalid");
        // A unique basename avoids observing another test's /tmp log files.
        let unique_name = harness.root.path().file_name().unwrap().to_string_lossy();
        request.game_path = harness
            .root
            .path()
            .join(format!("invalid-{unique_name}.exe"))
            .to_string_lossy()
            .into_owned();
        request.method = "unsupported-method".to_string();
        let slug = request.log_target_slug();
        assert!(logs_for_slug(&slug).is_empty());

        let error = if trainer {
            harness.service.start_trainer(request).await
        } else {
            harness.service.start_game(request).await
        }
        .expect_err("invalid request rejected");

        assert!(error.events.is_empty());
        assert!(harness
            .service
            .registry
            .active_profile_keys(None)
            .is_empty());
        assert!(harness.spawner.captured().is_empty());
        assert!(harness
            .service
            .launch_history("invalid".into(), None)
            .await
            .unwrap()
            .is_empty());
        assert!(
            logs_for_slug(&slug).is_empty(),
            "validation must precede log creation"
        );
    }
}

#[tokio::test]
async fn spawn_failures_deregister_without_history_and_replay_trainer_injections() {
    let harness = Harness::new(Metadata::Enabled);
    for trainer in [false, true] {
        let request = harness.proton_request("spawn-failure");
        harness
            .spawner
            .fail_next(io::Error::other("scripted spawn failure"));
        let error = if trainer {
            harness.service.start_trainer(request).await
        } else {
            harness.service.start_game(request).await
        }
        .expect_err("spawn fails");

        assert!(harness
            .service
            .registry
            .active_profile_keys(None)
            .is_empty());
        assert!(harness
            .service
            .launch_history("spawn-failure".into(), None)
            .await
            .unwrap()
            .is_empty());
        if trainer {
            assert_eq!(
                error.to_string(),
                "failed to launch trainer (method=proton_run): scripted spawn failure"
            );
            assert_eq!(error.events.len(), 2);
            let id = error.events[0].session_id();
            for (event, message, level) in [
                (
                    &error.events[0],
                    "Trainer launch requested.",
                    InjectionLevel::Info,
                ),
                (
                    &error.events[1],
                    "Trainer launch failed before the trainer process started.",
                    InjectionLevel::Error,
                ),
            ] {
                assert_eq!(event.session_id(), id);
                let LaunchEvent::Injection { event, .. } = event else {
                    panic!("spawn failure replays injection events only");
                };
                assert_eq!(event.message, message);
                assert_eq!(event.level, level);
                assert_eq!(event.session_kind, SessionKind::Trainer);
            }
        } else {
            assert_eq!(
                error.to_string(),
                "failed to launch helper: scripted spawn failure"
            );
            assert!(error.events.is_empty());
        }
        let captured = harness.spawner.captured();
        assert_eq!(captured.len(), usize::from(trainer) + 1);
    }
}

fn lines(events: &[LaunchEvent]) -> Vec<&str> {
    events
        .iter()
        .flat_map(|event| match event {
            LaunchEvent::Log {
                batch: LogBatch::Lines(lines),
                ..
            } => lines.iter().map(String::as_str).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_event_has_its_own_session_id_and_receivers_never_mix_lines() {
    let harness = Harness::new(Metadata::Enabled);
    harness.spawner.push(Script::piped(
        "printf 'game-only-line\\n'; printf 'game-error-line\\n' >&2; exit 7",
    ));
    harness.spawner.push(Script::piped(
        "printf 'trainer-only-line\\n'; printf 'trainer-error-line\\n' >&2; exit 8",
    ));
    let game = harness
        .service
        .start_game(harness.proton_request("game-events"))
        .await
        .unwrap();
    let trainer = harness
        .service
        .start_trainer(harness.proton_request("trainer-events"))
        .await
        .unwrap();
    let game_id = game.id;
    let trainer_id = trainer.id;
    assert_ne!(game_id, trainer_id);
    let game_log = game.log_path.clone();
    let trainer_log = trainer.log_path.clone();
    let (game_events, trainer_events) =
        tokio::join!(Harness::collect(game), Harness::collect(trainer));

    for (events, id) in [(&game_events, game_id), (&trainer_events, trainer_id)] {
        assert!(events.iter().all(|event| event.session_id() == id));
        assert!(matches!(events.last(), Some(LaunchEvent::Exited { .. })));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, LaunchEvent::Exited { .. }))
                .count(),
            1
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, LaunchEvent::Diagnostic { .. })),
            "nonzero exit surfaces a diagnostic"
        );
    }
    assert!(trainer_events
        .iter()
        .any(|event| matches!(event, LaunchEvent::Injection { .. })));
    assert!(!game_events
        .iter()
        .any(|event| matches!(event, LaunchEvent::Injection { .. })));
    let game_lines = lines(&game_events);
    let trainer_lines = lines(&trainer_events);
    assert!(game_lines.contains(&"game-only-line"));
    assert!(game_lines.contains(&"game-error-line"));
    assert!(trainer_lines.contains(&"trainer-only-line"));
    assert!(trainer_lines.contains(&"trainer-error-line"));
    assert!(game_lines.iter().all(|line| !line.contains("trainer-")));
    assert!(trainer_lines.iter().all(|line| !line.contains("game-")));
    fs::remove_file(game_log).unwrap();
    fs::remove_file(trainer_log).unwrap();
}

#[tokio::test]
async fn disabled_metadata_still_delivers_exactly_one_terminal_event_last() {
    let harness = Harness::new(Metadata::Disabled);
    for trainer in [false, true] {
        harness.spawner.push(Script::piped("printf 'last-line\\n'"));
        let request = harness.proton_request("disabled");
        let mut session = if trainer {
            harness.service.start_trainer(request).await
        } else {
            harness.service.start_game(request).await
        }
        .unwrap();
        let log_path = session.log_path.clone();
        let id = session.id;
        let events = tokio::time::timeout(Duration::from_secs(10), async {
            let mut events = Vec::new();
            while let Some(event) = session.events.recv().await {
                events.push(event);
            }
            events
        })
        .await
        .expect("stream closes after Exited");
        assert!(events.iter().all(|event| event.session_id() == id));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, LaunchEvent::Exited { .. }))
                .count(),
            1
        );
        assert!(matches!(
            events.last(),
            Some(LaunchEvent::Exited {
                code: Some(0),
                signal: None,
                ..
            })
        ));
        assert!(lines(&events).contains(&"last-line"));
        assert!(session.events.recv().await.is_none());
        fs::remove_file(log_path).unwrap();
    }
}

#[tokio::test]
async fn session_returns_created_log_path_and_appended_pre_hook_warning() {
    let harness = Harness::new(Metadata::Disabled);
    for trainer in [false, true] {
        let mut request = harness.proton_request("hook-warning");
        request.pre_launch_hooks.push(LaunchHook {
            id: "missing-hook".into(),
            name: "Missing hook".into(),
            enabled: true,
            stage: HookStage::PreLaunch,
            ..Default::default()
        });
        let session = if trainer {
            harness.service.start_trainer(request).await
        } else {
            harness.service.start_game(request).await
        }
        .unwrap();
        let log_path = session.log_path.clone();
        assert!(log_path.is_file());
        assert_eq!(log_path.parent(), Some(Path::new("/tmp/crosshook-logs")));
        let prefix = if trainer { "trainer-" } else { "game-" };
        assert!(log_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(prefix));
        assert_eq!(
            session.warnings.last().unwrap().code.as_deref(),
            Some("launch_hook_missing")
        );
        Harness::collect(session).await;
        fs::remove_file(log_path).unwrap();
    }
}

struct LinkedAtSpawn {
    registry: Arc<LaunchSessionRegistry>,
    parent: SessionId,
    inner: Arc<FakeSpawner>,
}

impl ProcessSpawner for LinkedAtSpawn {
    fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        let trainers = self
            .registry
            .sessions_for_profile("linked-at-spawn", Some(SessionKind::Trainer));
        assert_eq!(trainers.len(), 1, "trainer registered before spawning");
        assert_eq!(
            self.registry.link_to_parent(trainers[0], self.parent),
            Err(LinkError::AlreadyLinked),
            "parent link already installed before spawning"
        );
        self.inner.spawn(command)
    }
}

#[tokio::test]
async fn trainer_is_registered_and_linked_to_its_game_before_spawn() {
    let harness = Harness::new(Metadata::Disabled);
    let (parent, _cancel_rx) = harness
        .service
        .registry
        .register(SessionKind::Game, "linked-at-spawn");
    let service = harness
        .service
        .clone()
        .with_spawner(Arc::new(LinkedAtSpawn {
            registry: harness.service.registry.clone(),
            parent,
            inner: harness.spawner.clone(),
        }));
    let session = service
        .start_trainer(harness.proton_request("linked-at-spawn"))
        .await
        .unwrap();
    let log_path = session.log_path.clone();
    Harness::collect(session).await;
    harness.service.registry.deregister(parent);
    fs::remove_file(log_path).unwrap();
}
