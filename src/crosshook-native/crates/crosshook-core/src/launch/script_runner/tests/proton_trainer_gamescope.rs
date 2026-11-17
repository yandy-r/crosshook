//! Gamescope PID capture for trainers (YAN-1609).
//!
//! The service's `pid_capture_required` hands the watchdog
//! `<log>.gamescope.pid` for a Flatpak trainer wrapped in gamescope; these
//! tests pin the builder to write exactly that file in exactly that case.

use std::fs;
use std::path::{Path, PathBuf};

use super::support::write_executable_file;
use crate::launch::runtime_helpers::FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT;
use crate::launch::script_runner::gamescope_pid_capture_path;
use crate::launch::script_runner::proton_trainer::{
    build_proton_trainer_command_inner, flatpak_steam_trainer_request,
};
use crate::launch::{LaunchRequest, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::profile::GamescopeConfig;

struct Fixture {
    _temp_dir: tempfile::TempDir,
    request: LaunchRequest,
    log_path: PathBuf,
}

/// A Steam trainer request with an enabled trainer gamescope override.
/// `allow_nested` keeps the test independent of the runner's live gamescope.
fn steam_trainer_fixture(gamescope_enabled: bool) -> Fixture {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let compatdata_path = temp_dir.path().join("compatdata");
    let proton_path = temp_dir.path().join("proton");
    let steam_client_path = temp_dir.path().join("steam-client");
    fs::create_dir_all(compatdata_path.join("pfx/drive_c")).expect("compatdata dir");
    fs::create_dir_all(steam_client_path.join("steamapps")).expect("steamapps dir");
    fs::create_dir_all(steam_client_path.join("config")).expect("config dir");
    write_executable_file(&proton_path);

    let request = LaunchRequest {
        method: METHOD_STEAM_APPLAUNCH.to_string(),
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        trainer_path: "/opt/trainers/trainer.exe".to_string(),
        trainer_host_path: "/opt/trainers/trainer.exe".to_string(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: "12345".to_string(),
            compatdata_path: compatdata_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        trainer_gamescope: Some(GamescopeConfig {
            enabled: gamescope_enabled,
            allow_nested: true,
            ..Default::default()
        }),
        launch_trainer_only: true,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };
    let log_path = temp_dir.path().join("trainer-quest.log");
    Fixture {
        _temp_dir: temp_dir,
        request,
        log_path,
    }
}

fn args(command: &tokio::process::Command) -> Vec<String> {
    command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

/// The capture argument the PID-capture shell receives, if any.
fn capture_argument(args: &[String]) -> Option<&str> {
    let script = args
        .iter()
        .position(|arg| arg == FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT)?;
    // bash -c <script> bash <capture> gamescope ...
    assert_eq!(args.get(script + 1).map(String::as_str), Some("bash"));
    assert_eq!(args.get(script + 3).map(String::as_str), Some("gamescope"));
    args.get(script + 2).map(String::as_str)
}

fn expected_capture(log_path: &Path) -> String {
    gamescope_pid_capture_path(log_path)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn flatpak_steam_trainer_with_gamescope_writes_the_watchdog_capture_file() {
    let fixture = steam_trainer_fixture(true);
    let direct = flatpak_steam_trainer_request(&fixture.request);

    let command = build_proton_trainer_command_inner(&direct, &fixture.log_path, true, true)
        .expect("flatpak steam trainer command");

    assert_eq!(command.as_std().get_program(), "flatpak-spawn");
    let args = args(&command);
    assert_eq!(
        capture_argument(&args),
        Some(expected_capture(&fixture.log_path).as_str())
    );
}

#[test]
fn flatpak_proton_run_trainer_with_gamescope_writes_the_watchdog_capture_file() {
    let fixture = steam_trainer_fixture(true);
    let mut proton = fixture.request.clone();
    proton.method = METHOD_PROTON_RUN.to_string();
    proton.runtime.prefix_path = proton.steam.compatdata_path.clone();
    proton.runtime.proton_path = proton.steam.proton_path.clone();

    let command = build_proton_trainer_command_inner(&proton, &fixture.log_path, false, true)
        .expect("flatpak proton trainer command");

    let args = args(&command);
    assert_eq!(
        capture_argument(&args),
        Some(expected_capture(&fixture.log_path).as_str())
    );
}

#[test]
fn trainer_without_gamescope_writes_no_capture_file() {
    let fixture = steam_trainer_fixture(false);
    let direct = flatpak_steam_trainer_request(&fixture.request);

    let command = build_proton_trainer_command_inner(&direct, &fixture.log_path, true, true)
        .expect("flatpak steam trainer command");

    assert_eq!(capture_argument(&args(&command)), None);
}

#[test]
fn host_trainer_with_gamescope_writes_no_capture_file() {
    let fixture = steam_trainer_fixture(true);
    let direct = flatpak_steam_trainer_request(&fixture.request);

    let command = build_proton_trainer_command_inner(&direct, &fixture.log_path, true, false)
        .expect("host trainer command");

    assert_eq!(command.as_std().get_program(), "gamescope");
    assert_eq!(capture_argument(&args(&command)), None);
}

#[test]
fn flatpak_steam_trainer_request_runs_as_proton_run_on_the_steam_prefix() {
    let fixture = steam_trainer_fixture(true);

    let direct = flatpak_steam_trainer_request(&fixture.request);

    assert_eq!(direct.method, METHOD_PROTON_RUN);
    assert_eq!(
        direct.runtime.prefix_path,
        fixture.request.steam.compatdata_path
    );
    assert_eq!(
        direct.runtime.proton_path,
        fixture.request.steam.proton_path
    );
}
