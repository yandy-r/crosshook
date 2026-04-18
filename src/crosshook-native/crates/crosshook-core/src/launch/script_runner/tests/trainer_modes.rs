use super::support::{steam_request, write_executable_file};
use crate::launch::script_runner::{build_proton_trainer_command, build_trainer_command};
use crate::launch::LaunchRequest;

#[test]
fn proton_trainer_command_prefers_enabled_trainer_gamescope() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");

    std::fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
    std::fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    std::fs::write(&trainer_path, b"trainer").expect("trainer exe");
    write_executable_file(&proton_path);

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: temp_dir
            .path()
            .join("game.exe")
            .to_string_lossy()
            .into_owned(),
        trainer_path: trainer_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        gamescope: crate::profile::GamescopeConfig::default(),
        trainer_gamescope: Some(crate::profile::GamescopeConfig {
            enabled: true,
            internal_width: Some(1280),
            internal_height: Some(720),
            ..Default::default()
        }),
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        command.as_std().get_program().to_string_lossy(),
        "gamescope"
    );
    assert!(args.contains(&"-w".to_string()));
    assert!(args.contains(&"1280".to_string()));
    assert!(args.contains(&"-h".to_string()));
    assert!(args.contains(&"720".to_string()));
    assert!(args.contains(&"--".to_string()));
    assert!(args.contains(&proton_path.to_string_lossy().into_owned()));
}

#[test]
fn trainer_command_for_steam_inherits_enabled_game_gamescope_when_trainer_disabled() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-trainer.sh");
    let log_path = temp_dir.path().join("trainer.log");
    let mut request = steam_request();
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        ..Default::default()
    };
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig::default());

    let command =
        build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.contains(&"--gamescope-enabled".to_string()));
    assert!(!args.contains(&"--gamescope-inherit-runtime".to_string()));
    assert!(
        !args.contains(&"-f".to_string()),
        "auto-generated trainer gamescope should clear fullscreen: {args:?}"
    );
}

#[test]
fn trainer_command_for_steam_includes_allow_nested_when_trainer_gamescope_allows_nested() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-trainer.sh");
    let log_path = temp_dir.path().join("trainer.log");
    let mut request = steam_request();
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        fullscreen: true,
        ..Default::default()
    };
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
        enabled: true,
        allow_nested: true,
        fullscreen: true,
        ..Default::default()
    });

    let command =
        build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.contains(&"--gamescope-enabled".to_string()));
    assert!(args.contains(&"--gamescope-allow-nested".to_string()));
}

#[test]
fn proton_trainer_command_prepends_unshare_net_when_isolation_enabled() {
    if !crate::launch::runtime_helpers::is_unshare_net_available() {
        eprintln!("SKIP: unshare --net not available on this system");
        return;
    }

    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");

    std::fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
    std::fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    std::fs::write(&trainer_path, b"trainer").expect("trainer exe");
    write_executable_file(&proton_path);

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: temp_dir
            .path()
            .join("game.exe")
            .to_string_lossy()
            .into_owned(),
        trainer_path: trainer_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        network_isolation: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
    assert_eq!(
        command.as_std().get_program().to_string_lossy(),
        "unshare",
        "first program should be unshare when network_isolation is enabled"
    );
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args[0], "--net", "first arg should be --net");
}

#[test]
fn proton_trainer_command_skips_unshare_when_isolation_disabled() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");

    std::fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
    std::fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    std::fs::write(&trainer_path, b"trainer").expect("trainer exe");
    write_executable_file(&proton_path);

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: temp_dir
            .path()
            .join("game.exe")
            .to_string_lossy()
            .into_owned(),
        trainer_path: trainer_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        network_isolation: false,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert_ne!(
        program, "unshare",
        "should NOT use unshare when network_isolation is false"
    );
}
