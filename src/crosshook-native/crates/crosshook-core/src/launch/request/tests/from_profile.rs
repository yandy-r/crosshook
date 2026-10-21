use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use tokio::process::Command;

use super::support::write_executable_file;
use crate::launch::request::{
    LaunchRequest, LaunchRequestError, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crate::launch::runtime_helpers::host_environment_map;
use crate::launch::script_runner::{
    build_flatpak_steam_trainer_command, build_proton_trainer_command,
};
use crate::profile::{resolve_launch_method, GameProfile, GamescopeConfig};
use crate::settings::UmuPreference;

const STEAM_CLIENT: &str = "/home/user/.steam/root";

fn profile(method: &str) -> GameProfile {
    let mut profile = GameProfile::default();
    profile.game.executable_path = "/games/test/Game.exe".to_string();
    profile.trainer.path = "/trainers/Trainer.exe".to_string();
    profile.launch.method = method.to_string();
    profile.steam.app_id = "1245620".to_string();
    profile.steam.compatdata_path = "/steam/compatdata/1245620".to_string();
    profile.steam.proton_path = "/steam/proton/proton".to_string();
    profile.runtime.prefix_path = "/prefixes/test".to_string();
    profile.runtime.proton_path = "/runtime/proton/proton".to_string();
    profile.runtime.working_directory = "/games/test/bin".to_string();
    profile.launch.optimizations.enabled_option_ids = vec!["disable_steam_input".to_string()];
    profile
        .launch
        .custom_env_vars
        .insert("DXVK_HUD".to_string(), "fps".to_string());
    profile
}

fn build(profile: &GameProfile) -> LaunchRequest {
    LaunchRequest::from_profile(
        profile,
        resolve_launch_method(profile),
        STEAM_CLIENT,
        "test-game",
        UmuPreference::Auto,
    )
    .expect("request")
}

#[test]
fn from_profile_carries_steam_and_runtime_for_every_method() {
    for method in [METHOD_STEAM_APPLAUNCH, METHOD_PROTON_RUN, METHOD_NATIVE] {
        let source = profile(method);
        let request = build(&source);

        assert_eq!(request.method, method);
        assert_eq!(request.game_path, source.game.executable_path);
        assert_eq!(request.trainer_path, source.trainer.path);
        assert_eq!(request.trainer_host_path, source.trainer.path);
        assert_eq!(request.steam.app_id, "1245620");
        assert_eq!(request.steam.compatdata_path, "/steam/compatdata/1245620");
        assert_eq!(request.steam.proton_path, "/steam/proton/proton");
        assert_eq!(request.steam.steam_client_install_path, STEAM_CLIENT);
        assert_eq!(request.runtime.prefix_path, "/prefixes/test");
        assert_eq!(request.runtime.proton_path, "/runtime/proton/proton");
        assert_eq!(request.runtime.working_directory, "/games/test/bin");
        assert_eq!(
            request.optimizations.enabled_option_ids,
            ["disable_steam_input"]
        );
        assert_eq!(request.custom_env_vars, source.launch.custom_env_vars);
        assert_eq!(request.profile_name.as_deref(), Some("test-game"));
        assert!(request.network_isolation);
        assert!(!request.launch_game_only);
        assert!(!request.launch_trainer_only);
        assert_eq!(request.trainer_gamescope, None);
        assert_eq!(request.resolved_umu_game_id, None);
    }
}

#[test]
fn from_profile_trims_method() {
    let request = LaunchRequest::from_profile(
        &profile(""),
        " proton_run ",
        STEAM_CLIENT,
        "test-game",
        UmuPreference::Auto,
    )
    .expect("request");
    assert_eq!(request.method, METHOD_PROTON_RUN);
}

#[test]
fn from_profile_rejects_blank_game_path() {
    let mut source = profile(METHOD_PROTON_RUN);
    source.game.executable_path = "  ".to_string();
    assert_eq!(
        LaunchRequest::from_profile(
            &source,
            METHOD_PROTON_RUN,
            STEAM_CLIENT,
            "test-game",
            UmuPreference::Auto
        ),
        Err(LaunchRequestError::MissingGamePath)
    );
}

#[test]
fn from_profile_rejects_unsupported_method() {
    for method in ["", "wine"] {
        assert_eq!(
            LaunchRequest::from_profile(
                &profile(METHOD_PROTON_RUN),
                method,
                STEAM_CLIENT,
                "test-game",
                UmuPreference::Auto
            ),
            Err(LaunchRequestError::UnsupportedMethod(method.to_string()))
        );
    }
}

#[test]
fn from_profile_blank_name_is_none() {
    let request = LaunchRequest::from_profile(
        &profile(METHOD_NATIVE),
        METHOD_NATIVE,
        STEAM_CLIENT,
        " ",
        UmuPreference::Auto,
    )
    .expect("request");
    assert_eq!(request.profile_name, None);
}

#[test]
fn from_profile_umu_override_beats_default() {
    let mut source = profile(METHOD_PROTON_RUN);
    let request = LaunchRequest::from_profile(
        &source,
        METHOD_PROTON_RUN,
        STEAM_CLIENT,
        "test-game",
        UmuPreference::Proton,
    )
    .expect("request");
    assert_eq!(request.umu_preference, UmuPreference::Proton);

    source.runtime.umu_preference = Some(UmuPreference::Umu);
    let request = LaunchRequest::from_profile(
        &source,
        METHOD_PROTON_RUN,
        STEAM_CLIENT,
        "test-game",
        UmuPreference::Proton,
    )
    .expect("request");
    assert_eq!(request.umu_preference, UmuPreference::Umu);
}

#[test]
fn from_profile_carries_enabled_trainer_gamescope_override() {
    let mut source = profile(METHOD_PROTON_RUN);
    source.launch.trainer_gamescope = GamescopeConfig {
        enabled: true,
        output_width: Some(800),
        output_height: Some(400),
        ..Default::default()
    };
    let request = build(&source);
    assert_eq!(
        request.trainer_gamescope,
        Some(source.launch.trainer_gamescope.clone())
    );
}

#[derive(Debug, PartialEq)]
struct CommandShape {
    program: OsString,
    args: Vec<OsString>,
    env: BTreeMap<OsString, Option<OsString>>,
    host_passthrough_keys: Vec<OsString>,
    current_dir: Option<PathBuf>,
}

/// Command shape with host passthrough env (`host_environment_map` keys) removed: those
/// values are read from the live process env and can change mid-test, so only the
/// builder-set env is compared. Both commands must still pass through the same host keys.
fn shape(command: &Command) -> CommandShape {
    let std = command.as_std();
    let mut env: BTreeMap<OsString, Option<OsString>> = std
        .get_envs()
        .map(|(key, value)| (key.to_owned(), value.map(ToOwned::to_owned)))
        .collect();
    let host_passthrough_keys = host_environment_map()
        .into_keys()
        .map(OsString::from)
        .filter(|key| env.remove(key).is_some())
        .collect();
    CommandShape {
        program: std.get_program().to_owned(),
        args: std.get_args().map(ToOwned::to_owned).collect(),
        env,
        host_passthrough_keys,
        current_dir: std.get_current_dir().map(ToOwned::to_owned),
    }
}

/// A Steam profile's Flatpak trainer must execute exactly like an equivalent
/// `proton_run` profile's trainer: same env, working directory and gamescope.
#[test]
fn steam_flatpak_trainer_matches_equivalent_proton_run_trainer() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let compatdata = temp_dir.path().join("compatdata");
    let proton = temp_dir.path().join("proton");
    let working_directory = temp_dir.path().join("workdir");
    let log_path = temp_dir.path().join("trainer.log");
    std::fs::create_dir_all(compatdata.join("pfx/drive_c")).expect("compatdata");
    std::fs::create_dir_all(&working_directory).expect("workdir");
    write_executable_file(&proton);

    let compatdata = compatdata.to_string_lossy().into_owned();
    let proton = proton.to_string_lossy().into_owned();
    let trainer_gamescope = GamescopeConfig {
        enabled: true,
        output_width: Some(800),
        output_height: Some(400),
        allow_nested: true,
        ..Default::default()
    };

    let mut steam_profile = GameProfile::default();
    steam_profile.game.executable_path = "/games/test/Game.exe".to_string();
    steam_profile.trainer.path = "/trainers/Trainer.exe".to_string();
    steam_profile.launch.method = METHOD_STEAM_APPLAUNCH.to_string();
    steam_profile.steam.app_id = "1245620".to_string();
    steam_profile.steam.compatdata_path = compatdata.clone();
    steam_profile.steam.proton_path = proton.clone();
    steam_profile.runtime.working_directory = working_directory.to_string_lossy().into_owned();
    steam_profile.launch.trainer_gamescope = trainer_gamescope;
    steam_profile
        .launch
        .custom_env_vars
        .insert("DXVK_HUD".to_string(), "fps".to_string());

    // Flatpak Steam trainers bypass umu, so the proton_run twin opts out too.
    let mut proton_profile = steam_profile.clone();
    proton_profile.launch.method = METHOD_PROTON_RUN.to_string();
    proton_profile.runtime.prefix_path = compatdata;
    proton_profile.runtime.proton_path = proton;
    proton_profile.runtime.umu_preference = Some(UmuPreference::Proton);

    let mut steam_request = build(&steam_profile);
    steam_request.launch_trainer_only = true;
    let mut proton_request = build(&proton_profile);
    proton_request.launch_trainer_only = true;

    assert_eq!(
        steam_request.resolved_trainer_gamescope(),
        proton_request.resolved_trainer_gamescope()
    );
    let steam_command =
        build_flatpak_steam_trainer_command(&steam_request, &log_path).expect("steam trainer");
    let proton_command =
        build_proton_trainer_command(&proton_request, &log_path).expect("proton trainer");
    let steam_shape = shape(&steam_command);
    assert_eq!(steam_shape, shape(&proton_command));
    assert_eq!(steam_shape.current_dir, Some(working_directory));
}
