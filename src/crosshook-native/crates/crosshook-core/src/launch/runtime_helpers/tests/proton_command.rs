use std::collections::BTreeMap;
use std::path::Path;

use crate::launch::request::RuntimeLaunchConfig;
use crate::launch::runtime_helpers::proton_command::{
    build_direct_proton_command_lifecycle_with, build_direct_proton_command_with_wrappers,
    build_gamescope_args, build_proton_command_with_gamescope,
    build_proton_command_with_gamescope_pid_capture_in_directory_inner,
    collect_pressure_vessel_paths,
};
use crate::launch::runtime_helpers::FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT;
use crate::launch::LaunchRequest;
use crate::profile::{GamescopeConfig, GamescopeFilter, TrainerLoadingMode};

#[test]
fn build_gamescope_args_default_returns_empty() {
    let config = GamescopeConfig::default();
    let args = build_gamescope_args(&config);
    assert!(args.is_empty());
}

#[test]
fn build_gamescope_args_resolution_and_fps() {
    let config = GamescopeConfig {
        internal_width: Some(1280),
        internal_height: Some(800),
        output_width: Some(1920),
        output_height: Some(1080),
        frame_rate_limit: Some(60),
        ..Default::default()
    };
    let args = build_gamescope_args(&config);
    assert_eq!(
        args,
        vec!["-w", "1280", "-h", "800", "-W", "1920", "-H", "1080", "-r", "60"]
    );
}

#[test]
fn build_gamescope_args_all_flags() {
    let config = GamescopeConfig {
        fullscreen: true,
        borderless: true,
        grab_cursor: true,
        force_grab_cursor: true,
        hdr_enabled: true,
        fsr_sharpness: Some(5),
        upscale_filter: Some(GamescopeFilter::Fsr),
        ..Default::default()
    };
    let args = build_gamescope_args(&config);
    assert!(args.contains(&"--fsr-sharpness".to_string()));
    assert!(args.contains(&"5".to_string()));
    assert!(args.contains(&"--filter".to_string()));
    assert!(args.contains(&"fsr".to_string()));
    assert!(args.contains(&"-f".to_string()));
    assert!(args.contains(&"-b".to_string()));
    assert!(args.contains(&"--grab".to_string()));
    assert!(args.contains(&"--force-grab-cursor".to_string()));
    assert!(args.contains(&"--hdr-enabled".to_string()));
}

#[test]
fn build_gamescope_args_extra_args_passthrough() {
    let config = GamescopeConfig {
        extra_args: vec!["--expose-wayland".to_string(), "--rt".to_string()],
        ..Default::default()
    };
    let args = build_gamescope_args(&config);
    assert_eq!(args, vec!["--expose-wayland", "--rt"]);
}

#[test]
fn collect_pressure_vessel_paths_empty_request_returns_empty() {
    assert!(collect_pressure_vessel_paths(&LaunchRequest::default()).is_empty());
}

#[test]
fn collect_pressure_vessel_paths_game_trainer_working_dir_deduped() {
    let request = LaunchRequest {
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        trainer_host_path: "/opt/trainers/trainer.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        runtime: RuntimeLaunchConfig {
            working_directory: "/opt/games/TheGame".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        collect_pressure_vessel_paths(&request),
        vec![
            "/opt/games/TheGame".to_string(),
            "/opt/trainers".to_string(),
        ]
    );
}

#[test]
fn collect_pressure_vessel_paths_game_equals_working_dir_collapses() {
    let request = LaunchRequest {
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        runtime: RuntimeLaunchConfig {
            working_directory: "/opt/games/TheGame".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        collect_pressure_vessel_paths(&request),
        vec!["/opt/games/TheGame".to_string()]
    );
}

#[test]
fn collect_pressure_vessel_paths_copy_to_prefix_omits_trainer_dir() {
    let request = LaunchRequest {
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        trainer_host_path: "/opt/trainers/trainer.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        runtime: RuntimeLaunchConfig {
            working_directory: "/opt/games/TheGame".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        collect_pressure_vessel_paths(&request),
        vec!["/opt/games/TheGame".to_string()]
    );
}

#[test]
fn collect_pressure_vessel_paths_empty_trainer_host_path_source_directory_omits_entry() {
    let request = LaunchRequest {
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        runtime: RuntimeLaunchConfig {
            working_directory: "/opt/working".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        collect_pressure_vessel_paths(&request),
        vec!["/opt/games/TheGame".to_string(), "/opt/working".to_string()]
    );
}

#[test]
fn collect_pressure_vessel_paths_flatpak_host_prefix_normalized() {
    let request = LaunchRequest {
        game_path: "/run/host/opt/games/TheGame/game.exe".to_string(),
        trainer_host_path: "/run/host/opt/trainers/trainer.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        runtime: RuntimeLaunchConfig {
            working_directory: " /run/host/opt/games/TheGame ".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        collect_pressure_vessel_paths(&request),
        vec![
            "/opt/games/TheGame".to_string(),
            "/opt/trainers".to_string(),
        ]
    );
}

#[test]
fn collect_pressure_vessel_paths_root_directory_preserved() {
    let request = LaunchRequest {
        game_path: "/game.exe".to_string(),
        ..Default::default()
    };

    assert_eq!(
        collect_pressure_vessel_paths(&request),
        vec!["/".to_string()]
    );
}

#[test]
fn direct_proton_command_skips_empty_wrappers() {
    let command = build_direct_proton_command_with_wrappers(
        "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
        &["   ".to_string(), " \t ".to_string()],
        &BTreeMap::new(),
    );

    assert_eq!(
        command.as_std().get_program(),
        "/usr/share/steam/compatibilitytools.d/proton/proton"
    );
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(args, vec!["run".to_string()]);
}

#[test]
fn direct_proton_command_normalizes_wrappers_and_proton_path() {
    let command = build_direct_proton_command_with_wrappers(
        "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
        &[
            " /run/host/usr/bin/env ".to_string(),
            " /run/host/usr/bin/mangohud ".to_string(),
        ],
        &BTreeMap::new(),
    );

    assert_eq!(command.as_std().get_program(), "/usr/bin/env");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        args,
        vec![
            "/usr/bin/mangohud".to_string(),
            "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
            "run".to_string(),
        ]
    );
}

#[test]
fn gamescope_proton_command_normalizes_wrappers_and_proton_path() {
    let command = build_proton_command_with_gamescope(
        "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
        &[" /run/host/usr/bin/mangohud ".to_string()],
        &["-f".to_string()],
        &BTreeMap::new(),
    );

    assert_eq!(command.as_std().get_program(), "gamescope");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        args,
        vec![
            "-f".to_string(),
            "--".to_string(),
            "/usr/bin/mangohud".to_string(),
            "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
            "run".to_string(),
        ]
    );
}

#[test]
fn flatpak_gamescope_pid_capture_command_creates_parent_directory_on_host() {
    let command = build_proton_command_with_gamescope_pid_capture_in_directory_inner(
        "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
        &[" /run/host/usr/bin/mangohud ".to_string()],
        &["-f".to_string()],
        &BTreeMap::new(),
        None,
        &BTreeMap::new(),
        Some(Path::new("/tmp/crosshook-logs/game.gamescope.pid")),
        true,
        false,
    );

    assert_eq!(command.as_std().get_program(), "flatpak-spawn");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        args,
        vec![
            "--host".to_string(),
            "--clear-env".to_string(),
            "bash".to_string(),
            "-c".to_string(),
            FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT.to_string(),
            "bash".to_string(),
            "/tmp/crosshook-logs/game.gamescope.pid".to_string(),
            "gamescope".to_string(),
            "-f".to_string(),
            "--".to_string(),
            "/usr/bin/mangohud".to_string(),
            "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
            "run".to_string(),
        ]
    );
}

fn command_args(command: &tokio::process::Command) -> Vec<String> {
    command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn command_envs(command: &tokio::process::Command) -> BTreeMap<String, String> {
    command
        .as_std()
        .get_envs()
        .filter_map(|(key, value)| {
            value.map(|value| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
        })
        .collect()
}

fn lifecycle_env() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "STEAM_COMPAT_DATA_PATH".to_string(),
            "/games/compat".to_string(),
        ),
        ("WINEPREFIX".to_string(), "/games/compat/pfx".to_string()),
    ])
}

#[test]
fn lifecycle_proton_command_flatpak_watches_bus_and_ends_with_proton_run() {
    let env = lifecycle_env();
    let command = build_direct_proton_command_lifecycle_with(
        " /run/host/usr/share/steam/compatibilitytools.d/proton/proton ",
        &env,
        None,
        true,
    );

    assert_eq!(command.as_std().get_program(), "flatpak-spawn");
    assert_eq!(
        command_args(&command),
        vec![
            "--host".to_string(),
            "--watch-bus".to_string(),
            "--clear-env".to_string(),
            "--env=STEAM_COMPAT_DATA_PATH=/games/compat".to_string(),
            "--env=WINEPREFIX=/games/compat/pfx".to_string(),
            "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
            "run".to_string(),
        ]
    );
    assert!(command_envs(&command).is_empty());
}

#[test]
fn lifecycle_proton_command_flatpak_passes_directory_before_env() {
    let env = lifecycle_env();
    let command = build_direct_proton_command_lifecycle_with(
        "/opt/proton/proton",
        &env,
        Some("/run/host/mnt/games/Installer Dir"),
        true,
    );

    let args = command_args(&command);
    assert_eq!(
        &args[..4],
        &[
            "--host".to_string(),
            "--watch-bus".to_string(),
            "--clear-env".to_string(),
            "--directory=/mnt/games/Installer Dir".to_string(),
        ]
    );
    assert_eq!(
        &args[args.len() - 2..],
        &["/opt/proton/proton".to_string(), "run".to_string()]
    );
}

#[test]
fn lifecycle_proton_command_native_runs_proton_with_env_applied() {
    let env = lifecycle_env();
    let command =
        build_direct_proton_command_lifecycle_with("/opt/proton/proton", &env, None, false);

    assert_eq!(command.as_std().get_program(), "/opt/proton/proton");
    assert_eq!(command_args(&command), vec!["run".to_string()]);
    assert_eq!(command_envs(&command), env);
    assert_eq!(command.as_std().get_current_dir(), None);
}

#[test]
fn lifecycle_proton_command_native_sets_working_directory() {
    let env = lifecycle_env();
    let command = build_direct_proton_command_lifecycle_with(
        "/opt/proton/proton",
        &env,
        Some(" /tmp/installer "),
        false,
    );

    assert_eq!(command_args(&command), vec!["run".to_string()]);
    assert_eq!(
        command.as_std().get_current_dir(),
        Some(Path::new("/tmp/installer"))
    );
}

#[test]
fn lifecycle_proton_command_public_builder_ends_with_proton_run() {
    let command = crate::launch::runtime_helpers::build_direct_proton_command_lifecycle(
        "/opt/proton/proton",
        &BTreeMap::new(),
        None,
    );

    let args = command_args(&command);
    assert_eq!(args.last().map(String::as_str), Some("run"));
    assert!(args.iter().all(|arg| !arg.starts_with("--directory=")));
}
