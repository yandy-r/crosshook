#![cfg(test)]

use super::super::*;
use super::fixtures::*;
use crate::launch::optimizations::build_steam_launch_options_command;
use crate::launch::request::{LaunchRequest, METHOD_PROTON_RUN};
use crate::settings::UmuPreference;

#[test]
fn preview_includes_steam_launch_options() {
    let (_td, request) = steam_request();
    let preview = build_launch_preview(&request).expect("preview");

    // With no optimizations enabled, steam launch options should still be
    // populated (the bare "%command%" string).
    assert!(
        preview.steam_launch_options.is_some(),
        "expected steam_launch_options for steam_applaunch"
    );
    assert_eq!(preview.steam_launch_options.as_deref(), Some("%command%"));
}

#[test]
fn preview_surfaces_steam_launch_option_failures_without_fake_command() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let _scoped_path = crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

    let (_td, mut request) = steam_request();
    request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

    let preview = build_launch_preview(&request).expect("preview");

    assert!(preview.effective_command.is_none());
    assert!(preview.steam_launch_options.is_none());
    assert!(
        preview
            .directives_error
            .as_deref()
            .is_some_and(|error| error.contains("mangohud")),
        "expected directives_error to mention the missing wrapper, got {:?}",
        preview.directives_error
    );
}

#[test]
fn preview_steam_launch_options_string_matches_core_builder_with_custom_merge() {
    use std::collections::BTreeMap;

    let (_td, mut request) = steam_request();
    request.optimizations.enabled_option_ids = vec!["enable_dxvk_async".to_string()];
    request.custom_env_vars = BTreeMap::from([("DXVK_ASYNC".to_string(), "0".to_string())]);

    let preview = build_launch_preview(&request).expect("preview");
    let expected = build_steam_launch_options_command(
        &request.optimizations.enabled_option_ids,
        &request.custom_env_vars,
        None,
    )
    .expect("steam line");

    assert_eq!(
        preview.steam_launch_options.as_deref(),
        Some(expected.as_str())
    );

    let dxvk = preview
        .environment
        .as_ref()
        .expect("environment")
        .iter()
        .find(|v| v.key == "DXVK_ASYNC")
        .expect("DXVK_ASYNC");
    assert_eq!(dxvk.value, "0");
    assert_eq!(dxvk.source, EnvVarSource::ProfileCustom);
}

#[test]
fn preview_steam_gamescope_active_includes_gamescope_in_command() {
    let (_td, mut request) = steam_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        internal_width: Some(2560),
        internal_height: Some(1440),
        fullscreen: true,
        ..Default::default()
    };

    let preview = build_launch_preview(&request).expect("preview");
    let steam_opts = preview
        .steam_launch_options
        .as_deref()
        .expect("steam_launch_options");
    assert!(
        steam_opts.starts_with("gamescope"),
        "steam launch options should start with gamescope: {steam_opts}"
    );
    assert!(
        steam_opts.contains("-w 2560 -h 1440 -f"),
        "should contain gamescope args: {steam_opts}"
    );
    assert!(
        steam_opts.contains("-- %command%"),
        "should contain separator before %%command%%: {steam_opts}"
    );

    let effective = preview
        .effective_command
        .as_deref()
        .expect("effective_command");
    assert!(
        effective.starts_with("gamescope"),
        "effective command should also contain gamescope: {effective}"
    );
}

#[test]
fn preview_steam_gamescope_mangohud_swap() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mangohud_path = temp_dir.path().join("mangohud");
    write_executable_file(&mangohud_path);
    let _command_search_path =
        crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

    let (_td, mut request) = steam_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        ..Default::default()
    };
    request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

    let preview = build_launch_preview(&request).expect("preview");
    let steam_opts = preview
        .steam_launch_options
        .as_deref()
        .expect("steam_launch_options");
    assert!(
        steam_opts.contains("--mangoapp"),
        "should contain --mangoapp: {steam_opts}"
    );
    // mangohud should not appear as a separate wrapper token between -- and %command%
    let after_separator = steam_opts.split("-- ").last().unwrap_or("");
    assert!(
        !after_separator.contains("mangohud"),
        "mangohud should not appear as wrapper after --: {steam_opts}"
    );
}

#[test]
fn preview_trainer_only_uses_trainer_gamescope_and_trainer_path() {
    let (_td, mut request) = proton_request();
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    request.gamescope = crate::profile::GamescopeConfig::default();
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
        enabled: true,
        internal_width: Some(1024),
        internal_height: Some(576),
        ..Default::default()
    });

    let preview = build_launch_preview(&request).expect("preview");
    assert!(preview.gamescope_active);
    let command = preview
        .effective_command
        .as_deref()
        .expect("effective command");
    assert!(
        command.starts_with("gamescope"),
        "expected gamescope in: {command}"
    );
    assert!(
        command.contains(request.trainer_host_path.as_str()),
        "expected trainer host path in: {command}"
    );
    assert!(
        !command.contains(request.game_path.as_str()),
        "trainer-only command should not contain game path: {command}"
    );
}

#[test]
fn preview_trainer_only_falls_back_to_main_gamescope_when_trainer_disabled() {
    let (_td, mut request) = proton_request();
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..Default::default()
    };
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig::default());

    let preview = build_launch_preview(&request).expect("preview");
    assert!(
        preview.gamescope_active,
        "expected fallback gamescope to be active"
    );
    let command = preview
        .effective_command
        .as_deref()
        .expect("effective command");
    assert!(
        command.contains("-w 1920 -h 1080"),
        "expected auto-generated trainer gamescope resolution in: {command}"
    );
    assert!(
        !command.split_whitespace().any(|token| token == "-f"),
        "auto-generated trainer gamescope should not force fullscreen: {command}"
    );
}

#[test]
fn preview_trainer_only_auto_derives_windowed_gamescope_when_trainer_gamescope_is_none() {
    let (_td, mut request) = proton_request();
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        output_width: Some(1920),
        output_height: Some(1080),
        ..Default::default()
    };
    request.trainer_gamescope = None;

    let preview = build_launch_preview(&request).expect("preview");
    assert!(
        preview.gamescope_active,
        "expected auto-derived gamescope to be active"
    );
    let command = preview
        .effective_command
        .as_deref()
        .expect("effective command");
    assert!(
        command.contains("-W 1920 -H 1080"),
        "expected auto-derived trainer gamescope output resolution in: {command}"
    );
    assert!(
        !command.split_whitespace().any(|token| token == "-f"),
        "auto-derived trainer gamescope should not force fullscreen: {command}"
    );
}

#[test]
fn preview_command_string_uses_umu_run_when_use_umu() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    let mut request = LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Umu,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let preview = build_launch_preview(&request).unwrap();
    let command = preview.effective_command.unwrap();
    assert!(
        command.contains("umu-run"),
        "expected 'umu-run' in command, got: {command}"
    );
    assert!(
        !command.contains(" run /tmp/game.exe"),
        "no 'run' subcommand expected: {command}"
    );
}

#[test]
fn preview_proton_setup_umu_run_path_none_when_preference_is_proton() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    let mut request = LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Proton,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let preview = build_launch_preview(&request).unwrap();
    assert!(preview.proton_setup.unwrap().umu_run_path.is_none());
}
