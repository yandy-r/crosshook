use std::fs;

use super::support::{command_env_value, write_executable_file};
use crate::launch::script_runner::{
    build_flatpak_steam_trainer_command, build_proton_trainer_command,
};
use crate::launch::LaunchRequest;

#[test]
fn proton_trainer_command_ignores_game_optimization_wrappers_and_env() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let wrapper_dir = temp_dir.path().join("wrappers");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_host_path = trainer_source_dir.join("sample.exe");
    let trainer_support_path = trainer_source_dir.join("sample.ini");
    let log_path = temp_dir.path().join("trainer.log");
    let workspace_dir = prefix_path.join("workspace");
    let wine_prefix_path = prefix_path.join("pfx");

    fs::create_dir_all(&wrapper_dir).expect("wrapper dir");
    fs::create_dir_all(wine_prefix_path.join("drive_c")).expect("wine prefix dir");
    fs::create_dir_all(&trainer_source_dir).expect("trainer dir");
    fs::create_dir_all(&workspace_dir).expect("workspace dir");
    write_executable_file(&wrapper_dir.join("mangohud"));
    write_executable_file(&wrapper_dir.join("gamemoderun"));
    write_executable_file(&proton_path);
    fs::write(&trainer_host_path, b"trainer").expect("trainer exe");
    fs::write(&trainer_support_path, b"config").expect("trainer config");

    let _command_search_path =
        crate::launch::test_support::ScopedCommandSearchPath::new(&wrapper_dir);
    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: temp_dir
            .path()
            .join("game.exe")
            .to_string_lossy()
            .into_owned(),
        trainer_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: workspace_dir.to_string_lossy().into_owned(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest {
            enabled_option_ids: vec![
                "disable_steam_input".to_string(),
                "show_mangohud_overlay".to_string(),
                "use_gamemode".to_string(),
            ],
        },
        launch_trainer_only: true,
        launch_game_only: false,
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
        proton_path.to_string_lossy()
    );
    assert_eq!(
        args,
        vec![
            "run".to_string(),
            "C:\\CrossHook\\StagedTrainers\\sample\\sample.exe".to_string(),
        ]
    );
    assert_eq!(
        command
            .as_std()
            .get_current_dir()
            .map(|path| path.to_string_lossy().into_owned()),
        Some(workspace_dir.to_string_lossy().into_owned())
    );
    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
        Some(prefix_path.to_string_lossy().into_owned())
    );
    assert_eq!(
        command_env_value(&command, "WINEPREFIX"),
        Some(wine_prefix_path.to_string_lossy().into_owned())
    );
    assert_eq!(command_env_value(&command, "GAMEID"), None);
    assert_eq!(command_env_value(&command, "PROTON_NO_STEAMINPUT"), None);
    assert_eq!(command_env_value(&command, "DXVK_ASYNC"), None);
    assert_eq!(command_env_value(&command, "MANGOHUD_CONFIGFILE"), None);
    assert!(wine_prefix_path
        .join("drive_c/CrossHook/StagedTrainers/sample/sample.ini")
        .exists());
}

#[test]
fn proton_trainer_command_sets_proton_verb_to_runinprefix() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_host_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");
    let workspace_dir = prefix_path.join("workspace");

    fs::create_dir_all(&trainer_source_dir).expect("trainer dir");
    fs::create_dir_all(&workspace_dir).expect("workspace dir");
    write_executable_file(&proton_path);
    fs::write(&trainer_host_path, b"trainer").expect("trainer exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: String::new(),
        trainer_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: workspace_dir.to_string_lossy().into_owned(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
    assert_eq!(command_env_value(&command, "PROTON_VERB"), None);
}

#[test]
fn proton_trainer_command_sets_pressure_vessel_paths_skipping_copy_to_prefix_trainer_dir() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_host_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");

    fs::create_dir_all(trainer_source_dir).expect("trainer dir");
    fs::create_dir_all(prefix_path.join("pfx/drive_c")).expect("prefix dir");
    write_executable_file(&proton_path);
    fs::write(&trainer_host_path, b"trainer").expect("trainer exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        trainer_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: "/srv/crosshook/workspaces/the-game".to_string(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_trainer_command(&request, &log_path).expect("trainer command");
    let expected = "/opt/games/TheGame:/srv/crosshook/workspaces/the-game".to_string();

    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_LIBRARY_PATHS"),
        Some(expected.clone())
    );
    assert_eq!(
        command_env_value(&command, "PRESSURE_VESSEL_FILESYSTEMS_RW"),
        Some(expected)
    );
}

#[test]
fn flatpak_steam_trainer_command_inherits_proton_verb_runinprefix() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_host_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");
    let workspace_dir = prefix_path.join("workspace");

    fs::create_dir_all(&trainer_source_dir).expect("trainer dir");
    fs::create_dir_all(&workspace_dir).expect("workspace dir");
    write_executable_file(&proton_path);
    fs::write(&trainer_host_path, b"trainer").expect("trainer exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: String::new(),
        trainer_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_host_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: String::new(),
            compatdata_path: String::new(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            steam_client_install_path: String::new(),
        },
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: workspace_dir.to_string_lossy().into_owned(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command =
        build_flatpak_steam_trainer_command(&request, &log_path).expect("flatpak trainer command");
    assert_eq!(command_env_value(&command, "PROTON_VERB"), None);
}

#[test]
fn flatpak_steam_trainer_command_inherits_pressure_vessel_allowlist() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let compatdata_path = temp_dir.path().join("compatdata");
    let proton_path = temp_dir.path().join("proton");
    let steam_client_path = temp_dir.path().join("steam-client");
    let log_path = temp_dir.path().join("trainer.log");

    fs::create_dir_all(compatdata_path.join("pfx/drive_c")).expect("compatdata dir");
    fs::create_dir_all(steam_client_path.join("steamapps")).expect("steam client steamapps dir");
    fs::create_dir_all(steam_client_path.join("config")).expect("steam client config dir");
    write_executable_file(&proton_path);

    let request = LaunchRequest {
        method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
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
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: String::new(),
            proton_path: String::new(),
            working_directory: "/srv/crosshook/workspaces/the-game".to_string(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        profile_name: None,
        ..Default::default()
    };

    let command =
        build_flatpak_steam_trainer_command(&request, &log_path).expect("flatpak trainer command");
    let expected =
        "/opt/games/TheGame:/opt/trainers:/srv/crosshook/workspaces/the-game".to_string();

    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_LIBRARY_PATHS"),
        Some(expected.clone())
    );
    assert_eq!(
        command_env_value(&command, "PRESSURE_VESSEL_FILESYSTEMS_RW"),
        Some(expected)
    );
}
