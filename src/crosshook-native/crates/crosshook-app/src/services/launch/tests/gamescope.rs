//! AC1: the gamescope predicate table, the trainer config rules and the
//! cancel-consumer choice.
//!
//! Expected values are literals transcribed from `research-business.md` §2
//! (never recomputed with the predicate's own expression).

use std::path::{Path, PathBuf};
use std::time::Duration;

use crosshook_core::launch::{
    session_profile_key_for_name, LaunchRequest, LaunchSessionRegistry, SessionKind,
    TeardownReason, WatchdogOutcome, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::profile::GamescopeConfig;

use super::super::gamescope::{
    choose_cancel_consumer, execution_method, gamescope_plan, gamescope_required,
    pid_capture_required, spawn_cancel_consumer, CancelConsumer, GamescopePlan,
};
use super::super::LaunchEnv;

/// The game gamescope setting of a table row.
#[derive(Debug, Clone, Copy)]
enum Config {
    Disabled,
    Enabled,
    EnabledAllowNested,
}

/// The six `(config, inside_gamescope_session)` combinations, in the order the
/// per-row arrays below are written.
const COMBOS: [(Config, bool); 6] = [
    (Config::Disabled, false),
    (Config::Disabled, true),
    (Config::Enabled, false),
    (Config::Enabled, true),
    (Config::EnabledAllowNested, false),
    (Config::EnabledAllowNested, true),
];

/// One `(method, kind, flatpak)` block of the truth table.
struct Block {
    method: &'static str,
    kind: SessionKind,
    flatpak: bool,
    exec: &'static str,
    /// `gamescope_required` for each entry of [`COMBOS`].
    required: [bool; 6],
    /// `pid_capture_required` for each entry of [`COMBOS`].
    capture: [bool; 6],
}

const NEVER: [bool; 6] = [false; 6];
/// proton_run exec: wrapped when enabled and (nested allowed or not inside).
const WRAPPED: [bool; 6] = [false, false, true, false, true, true];

const TABLE: [Block; 12] = [
    // Game, steam_applaunch: CrossHook never wraps the Steam game.
    Block {
        method: METHOD_STEAM_APPLAUNCH,
        kind: SessionKind::Game,
        flatpak: false,
        exec: METHOD_STEAM_APPLAUNCH,
        required: NEVER,
        capture: NEVER,
    },
    Block {
        method: METHOD_STEAM_APPLAUNCH,
        kind: SessionKind::Game,
        flatpak: true,
        exec: METHOD_STEAM_APPLAUNCH,
        required: NEVER,
        capture: NEVER,
    },
    // Game, proton_run.
    Block {
        method: METHOD_PROTON_RUN,
        kind: SessionKind::Game,
        flatpak: false,
        exec: METHOD_PROTON_RUN,
        required: WRAPPED,
        capture: NEVER,
    },
    Block {
        method: METHOD_PROTON_RUN,
        kind: SessionKind::Game,
        flatpak: true,
        exec: METHOD_PROTON_RUN,
        required: WRAPPED,
        capture: WRAPPED,
    },
    // Game, native: the native builder ignores gamescope.
    Block {
        method: METHOD_NATIVE,
        kind: SessionKind::Game,
        flatpak: false,
        exec: METHOD_NATIVE,
        required: NEVER,
        capture: NEVER,
    },
    Block {
        method: METHOD_NATIVE,
        kind: SessionKind::Game,
        flatpak: true,
        exec: METHOD_NATIVE,
        required: NEVER,
        capture: NEVER,
    },
    // Trainer, steam_applaunch: script-side gamescope outside Flatpak; the
    // direct Proton builder (and so the watchdog) under Flatpak.
    Block {
        method: METHOD_STEAM_APPLAUNCH,
        kind: SessionKind::Trainer,
        flatpak: false,
        exec: METHOD_STEAM_APPLAUNCH,
        required: NEVER,
        capture: NEVER,
    },
    Block {
        method: METHOD_STEAM_APPLAUNCH,
        kind: SessionKind::Trainer,
        flatpak: true,
        exec: METHOD_PROTON_RUN,
        required: WRAPPED,
        capture: WRAPPED,
    },
    // Trainer, proton_run. Flatpak capture is a kept divergence: the path
    // reaches the watchdog but the trainer builder never writes the file.
    Block {
        method: METHOD_PROTON_RUN,
        kind: SessionKind::Trainer,
        flatpak: false,
        exec: METHOD_PROTON_RUN,
        required: WRAPPED,
        capture: NEVER,
    },
    Block {
        method: METHOD_PROTON_RUN,
        kind: SessionKind::Trainer,
        flatpak: true,
        exec: METHOD_PROTON_RUN,
        required: WRAPPED,
        capture: WRAPPED,
    },
    // Trainer, native: rejected by validation; the predicate stays false.
    Block {
        method: METHOD_NATIVE,
        kind: SessionKind::Trainer,
        flatpak: false,
        exec: METHOD_NATIVE,
        required: NEVER,
        capture: NEVER,
    },
    Block {
        method: METHOD_NATIVE,
        kind: SessionKind::Trainer,
        flatpak: true,
        exec: METHOD_NATIVE,
        required: NEVER,
        capture: NEVER,
    },
];

fn gamescope(config: Config) -> GamescopeConfig {
    match config {
        Config::Disabled => GamescopeConfig::default(),
        Config::Enabled => GamescopeConfig {
            enabled: true,
            ..Default::default()
        },
        Config::EnabledAllowNested => GamescopeConfig {
            enabled: true,
            allow_nested: true,
            ..Default::default()
        },
    }
}

fn request(method: &str, config: Config) -> LaunchRequest {
    LaunchRequest {
        method: method.to_string(),
        game_path: "/games/Quest/game.exe".to_string(),
        trainer_host_path: "/trainers/quest-trainer.exe".to_string(),
        gamescope: gamescope(config),
        ..Default::default()
    }
}

fn env(is_flatpak: bool, inside_gamescope_session: bool) -> LaunchEnv {
    LaunchEnv {
        is_flatpak,
        inside_gamescope_session,
    }
}

fn enabled_game_request() -> LaunchRequest {
    request(METHOD_PROTON_RUN, Config::Enabled)
}

#[test]
fn truth_table_matches_execution_method_required_and_capture() {
    for block in &TABLE {
        for (index, (config, inside)) in COMBOS.iter().enumerate() {
            let req = request(block.method, *config);
            let host = env(block.flatpak, *inside);
            let row = format!(
                "method={} kind={:?} flatpak={} config={:?} inside={}",
                block.method, block.kind, block.flatpak, config, inside
            );

            assert_eq!(
                execution_method(&req, block.kind, host),
                block.exec,
                "execution_method: {row}"
            );
            assert_eq!(
                gamescope_required(&req, block.kind, host),
                block.required[index],
                "gamescope_required: {row}"
            );
            assert_eq!(
                pid_capture_required(&req, block.kind, host),
                block.capture[index],
                "pid_capture_required: {row}"
            );
        }
    }
}

#[test]
fn table_covers_every_method_kind_and_flatpak_combination() {
    assert_eq!(TABLE.len(), 3 * 2 * 2);
    for method in [METHOD_STEAM_APPLAUNCH, METHOD_PROTON_RUN, METHOD_NATIVE] {
        for kind in [SessionKind::Game, SessionKind::Trainer] {
            for flatpak in [false, true] {
                assert_eq!(
                    TABLE
                        .iter()
                        .filter(|b| b.method == method && b.kind == kind && b.flatpak == flatpak)
                        .count(),
                    1,
                    "missing block: {method} {kind:?} flatpak={flatpak}"
                );
            }
        }
    }
}

#[test]
fn execution_method_resolves_a_blank_method_like_the_request() {
    let host = env(false, false);

    let mut with_app_id = LaunchRequest::default();
    with_app_id.steam.app_id = "12345".to_string();
    assert_eq!(
        execution_method(&with_app_id, SessionKind::Game, host),
        METHOD_STEAM_APPLAUNCH
    );

    let windows_exe = LaunchRequest {
        game_path: "/games/Quest/GAME.EXE".to_string(),
        ..Default::default()
    };
    assert_eq!(
        execution_method(&windows_exe, SessionKind::Game, host),
        METHOD_PROTON_RUN
    );

    let plain = LaunchRequest {
        game_path: "/games/quest.sh".to_string(),
        ..Default::default()
    };
    assert_eq!(
        execution_method(&plain, SessionKind::Game, host),
        METHOD_NATIVE
    );
}

#[test]
fn blank_method_steam_game_makes_a_flatpak_trainer_proton_run() {
    let mut req = LaunchRequest::default();
    req.steam.app_id = "12345".to_string();
    req.gamescope = gamescope(Config::Enabled);

    assert_eq!(
        execution_method(&req, SessionKind::Trainer, env(true, false)),
        METHOD_PROTON_RUN
    );
    assert_eq!(
        execution_method(&req, SessionKind::Trainer, env(false, false)),
        METHOD_STEAM_APPLAUNCH
    );
    assert!(gamescope_required(
        &req,
        SessionKind::Trainer,
        env(true, false)
    ));
    assert!(!gamescope_required(
        &req,
        SessionKind::Game,
        env(true, false)
    ));
}

#[test]
fn trainer_uses_an_enabled_override_even_when_the_game_gamescope_is_off() {
    let mut req = request(METHOD_PROTON_RUN, Config::Disabled);
    req.trainer_gamescope = Some(gamescope(Config::Enabled));
    let host = env(false, false);

    assert!(gamescope_required(&req, SessionKind::Trainer, host));
    // The game session keeps using the game config.
    assert!(!gamescope_required(&req, SessionKind::Game, host));
}

#[test]
fn trainer_override_is_used_as_is_so_allow_nested_comes_from_the_override() {
    let mut req = request(METHOD_PROTON_RUN, Config::EnabledAllowNested);
    req.trainer_gamescope = Some(gamescope(Config::Enabled));
    let inside = env(false, true);

    // The override (allow_nested=false) wins over the nested-friendly game
    // config, so the trainer skips gamescope inside a session.
    assert!(!gamescope_required(&req, SessionKind::Trainer, inside));
    assert!(gamescope_required(&req, SessionKind::Game, inside));
}

#[test]
fn trainer_derives_from_an_enabled_game_config_when_there_is_no_override() {
    let mut req = request(METHOD_PROTON_RUN, Config::EnabledAllowNested);
    req.gamescope.fullscreen = true;
    req.gamescope.borderless = true;

    // The windowed derivation keeps `enabled` and `allow_nested`.
    assert!(gamescope_required(
        &req,
        SessionKind::Trainer,
        env(false, true)
    ));
    assert!(gamescope_required(
        &req,
        SessionKind::Trainer,
        env(false, false)
    ));
}

#[test]
fn trainer_ignores_a_disabled_override_when_the_game_config_is_enabled() {
    let mut req = request(METHOD_PROTON_RUN, Config::Enabled);
    req.trainer_gamescope = Some(gamescope(Config::Disabled));

    assert!(gamescope_required(
        &req,
        SessionKind::Trainer,
        env(false, false)
    ));
}

#[test]
fn trainer_has_no_gamescope_when_neither_config_is_enabled() {
    let mut req = request(METHOD_PROTON_RUN, Config::Disabled);
    req.trainer_gamescope = Some(gamescope(Config::Disabled));

    for inside in [false, true] {
        assert!(!gamescope_required(
            &req,
            SessionKind::Trainer,
            env(false, inside)
        ));
    }
}

#[test]
fn plan_carries_the_pid_capture_path_only_when_required_in_flatpak() {
    let req = enabled_game_request();
    let log = Path::new("/tmp/crosshook-logs/game-quest-1.log");

    let native = gamescope_plan(&req, SessionKind::Game, env(false, false), log);
    assert_eq!(
        native,
        GamescopePlan {
            required: true,
            pid_capture: None
        }
    );

    let flatpak = gamescope_plan(&req, SessionKind::Game, env(true, false), log);
    assert_eq!(
        flatpak,
        GamescopePlan {
            required: true,
            pid_capture: Some(PathBuf::from(
                "/tmp/crosshook-logs/game-quest-1.log.gamescope.pid"
            )),
        }
    );

    let skipped = gamescope_plan(&req, SessionKind::Game, env(true, true), log);
    assert_eq!(
        skipped,
        GamescopePlan {
            required: false,
            pid_capture: None
        }
    );
}

fn required_plan(pid_capture: Option<&str>) -> GamescopePlan {
    GamescopePlan {
        required: true,
        pid_capture: pid_capture.map(Into::into),
    }
}

const NOT_REQUIRED: GamescopePlan = GamescopePlan {
    required: false,
    pid_capture: None,
};

#[test]
fn consumer_is_drain_when_the_predicate_is_false() {
    let req = enabled_game_request();
    for kind in [SessionKind::Game, SessionKind::Trainer] {
        assert_eq!(
            choose_cancel_consumer(&NOT_REQUIRED, &req, kind, Some(42)),
            CancelConsumer::Drain
        );
    }
}

#[test]
fn consumer_is_drain_when_the_child_has_no_pid() {
    let req = enabled_game_request();
    for kind in [SessionKind::Game, SessionKind::Trainer] {
        assert_eq!(
            choose_cancel_consumer(&required_plan(None), &req, kind, None),
            CancelConsumer::Drain
        );
    }
}

#[test]
fn game_watchdog_waits_on_the_game_path_basename() {
    let req = enabled_game_request();

    assert_eq!(
        choose_cancel_consumer(&required_plan(None), &req, SessionKind::Game, Some(7)),
        CancelConsumer::Watchdog {
            pid: 7,
            exe_name: "game.exe".to_string(),
            capture: None,
        }
    );
}

#[test]
fn trainer_watchdog_waits_on_the_trainer_host_path_basename() {
    let req = enabled_game_request();

    assert_eq!(
        choose_cancel_consumer(&required_plan(None), &req, SessionKind::Trainer, Some(8)),
        CancelConsumer::Watchdog {
            pid: 8,
            exe_name: "quest-trainer.exe".to_string(),
            capture: None,
        }
    );
}

#[test]
fn watchdog_receives_the_plan_capture_path() {
    let req = enabled_game_request();
    let plan = required_plan(Some("/tmp/crosshook-logs/game-quest-1.log.gamescope.pid"));

    assert_eq!(
        choose_cancel_consumer(&plan, &req, SessionKind::Game, Some(9)),
        CancelConsumer::Watchdog {
            pid: 9,
            exe_name: "game.exe".to_string(),
            capture: Some("/tmp/crosshook-logs/game-quest-1.log.gamescope.pid".into()),
        }
    );
}

#[test]
fn exe_name_is_empty_when_the_path_has_no_file_name() {
    let mut req = enabled_game_request();
    req.game_path = String::new();
    req.trainer_host_path = "/".to_string();

    for kind in [SessionKind::Game, SessionKind::Trainer] {
        assert_eq!(
            choose_cancel_consumer(&required_plan(None), &req, kind, Some(1)),
            CancelConsumer::Watchdog {
                pid: 1,
                exe_name: String::new(),
                capture: None,
            }
        );
    }
}

#[test]
fn the_game_exe_name_is_not_used_for_a_trainer_and_vice_versa() {
    let mut req = enabled_game_request();
    req.trainer_host_path = String::new();

    assert_eq!(
        choose_cancel_consumer(&required_plan(None), &req, SessionKind::Trainer, Some(3)),
        CancelConsumer::Watchdog {
            pid: 3,
            exe_name: String::new(),
            capture: None,
        }
    );
    assert_eq!(
        choose_cancel_consumer(&required_plan(None), &req, SessionKind::Game, Some(3)),
        CancelConsumer::Watchdog {
            pid: 3,
            exe_name: "game.exe".to_string(),
            capture: None,
        }
    );
}

#[tokio::test]
async fn drain_consumer_records_the_cancel_reason_without_a_kill() {
    let registry = LaunchSessionRegistry::new();
    let (id, rx) = registry.register(
        SessionKind::Game,
        session_profile_key_for_name(Some("quest")),
    );
    let outcome = WatchdogOutcome::new();

    spawn_cancel_consumer(CancelConsumer::Drain, id, outcome.clone(), rx, None);
    assert!(registry.cancel_session(id, TeardownReason::UserRequest));

    tokio::time::timeout(Duration::from_secs(5), async {
        while outcome.reason().is_none() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("drain records the reason");
    assert_eq!(outcome.reason(), Some(TeardownReason::UserRequest));
    assert!(!outcome.was_killed());
}

#[tokio::test]
async fn watchdog_with_an_empty_exe_name_drops_the_cancel_channel() {
    // Kept divergence (D8, I-32): nothing consumes the receiver, so the
    // cancel is never attributed.
    let registry = LaunchSessionRegistry::new();
    let (id, rx) = registry.register(
        SessionKind::Trainer,
        session_profile_key_for_name(Some("quest")),
    );
    let outcome = WatchdogOutcome::new();

    spawn_cancel_consumer(
        CancelConsumer::Watchdog {
            pid: 1,
            exe_name: String::new(),
            capture: None,
        },
        id,
        outcome.clone(),
        rx,
        None,
    );
    assert!(registry.cancel_session(id, TeardownReason::LinkedSessionExit));
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(outcome.reason(), None);
    assert!(!outcome.was_killed());
}
