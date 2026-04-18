use std::fs;
use std::path::PathBuf;

use crate::launch::request::{LaunchOptimizationsRequest, LaunchRequest, METHOD_PROTON_RUN};

use super::support::proton_request;

#[test]
fn resolved_trainer_gamescope_uses_explicit_when_enabled() {
    let (_temp_dir, mut request) = proton_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..Default::default()
    };
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: false,
        internal_width: Some(800),
        internal_height: Some(600),
        ..Default::default()
    });

    let resolved = request.resolved_trainer_gamescope();

    assert!(resolved.enabled);
    assert!(!resolved.fullscreen);
    assert_eq!(resolved.internal_width, Some(800));
    assert_eq!(resolved.internal_height, Some(600));
}

#[test]
fn resolved_trainer_gamescope_auto_generates_windowed_from_game() {
    let (_temp_dir, mut request) = proton_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        borderless: false,
        internal_width: Some(1920),
        internal_height: Some(1080),
        frame_rate_limit: Some(60),
        fsr_sharpness: Some(5),
        hdr_enabled: true,
        grab_cursor: true,
        extra_args: vec!["--custom".to_string()],
        ..Default::default()
    };
    request.trainer_gamescope = None;

    let resolved = request.resolved_trainer_gamescope();

    assert!(resolved.enabled, "auto-generated config should be enabled");
    assert!(
        !resolved.fullscreen,
        "auto-generated config must not be fullscreen"
    );
    assert!(
        !resolved.borderless,
        "auto-generated config must not be borderless"
    );
    assert_eq!(resolved.internal_width, Some(1920));
    assert_eq!(resolved.internal_height, Some(1080));
    assert_eq!(resolved.frame_rate_limit, Some(60));
    assert_eq!(resolved.fsr_sharpness, Some(5));
    assert!(resolved.hdr_enabled);
    assert!(resolved.grab_cursor);
    assert_eq!(resolved.extra_args, vec!["--custom"]);
}

#[test]
fn resolved_trainer_gamescope_returns_default_when_game_disabled() {
    let (_temp_dir, mut request) = proton_request();
    request.gamescope = crate::profile::GamescopeConfig::default();
    request.trainer_gamescope = None;

    let resolved = request.resolved_trainer_gamescope();

    assert_eq!(resolved, crate::profile::GamescopeConfig::default());
}

#[test]
fn resolved_trainer_gamescope_ignores_disabled_explicit_override() {
    let (_temp_dir, mut request) = proton_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        internal_width: Some(1920),
        ..Default::default()
    };
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
        enabled: false,
        fullscreen: true,
        internal_width: Some(800),
        ..Default::default()
    });

    let resolved = request.resolved_trainer_gamescope();

    assert!(resolved.enabled);
    assert!(!resolved.fullscreen);
    assert_eq!(resolved.internal_width, Some(1920));
}

#[test]
fn request_uses_last_path_segment_for_executable_name() {
    let request = LaunchRequest {
        game_path: r"Z:\Games\Test Game\game.exe".to_string(),
        optimizations: LaunchOptimizationsRequest::default(),
        ..LaunchRequest::default()
    };

    assert_eq!(request.game_executable_name(), "game.exe");
}

#[test]
fn log_target_slug_prefers_game_name_for_non_steam_methods() {
    let request = LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: "/games/Example Game/game.exe".to_string(),
        optimizations: LaunchOptimizationsRequest::default(),
        ..LaunchRequest::default()
    };

    assert_eq!(request.log_target_slug(), "game-exe");
}

#[test]
fn native_request_fixture_can_be_mutated_to_windows_exe() {
    let (_temp_dir, mut request) = super::support::native_request();
    let mut game_path = PathBuf::from(&request.game_path);
    game_path.set_extension("exe");
    request.game_path = game_path.to_string_lossy().into_owned();
    fs::write(&request.game_path, b"game").expect("game exe");

    assert!(request.game_path.ends_with(".exe"));
}
