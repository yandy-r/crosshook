use std::fs;

use super::support::{command_env_value, write_executable_file};
use crate::launch::script_runner::{
    build_flatpak_steam_trainer_command, build_proton_trainer_command,
};
use crate::launch::LaunchRequest;

#[test]
fn flatpak_steam_trainer_command_reuses_direct_proton_builder_inputs() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let compatdata_path = temp_dir.path().join("compatdata");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("sample.exe");
    let steam_client_path = temp_dir.path().join("steam-client");
    let log_path = temp_dir.path().join("trainer.log");

    fs::create_dir_all(compatdata_path.join("pfx/drive_c")).expect("compatdata dir");
    fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    fs::create_dir_all(steam_client_path.join("steamapps")).expect("steam client steamapps dir");
    fs::create_dir_all(steam_client_path.join("config")).expect("steam client config dir");
    write_executable_file(&proton_path);
    fs::write(&trainer_path, b"trainer").expect("trainer exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
        game_path: temp_dir
            .path()
            .join("game.exe")
            .to_string_lossy()
            .into_owned(),
        trainer_path: trainer_path.to_string_lossy().into_owned(),
        trainer_host_path: trainer_path.to_string_lossy().into_owned(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: "12345".to_string(),
            compatdata_path: compatdata_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        runtime: crate::launch::RuntimeLaunchConfig::default(),
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: true,
        launch_game_only: false,
        profile_name: None,
        ..Default::default()
    };

    let command = build_flatpak_steam_trainer_command(&request, &log_path)
        .expect("flatpak steam trainer command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        args,
        vec![
            "run".to_string(),
            trainer_path.to_string_lossy().into_owned()
        ]
    );
    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
        Some(compatdata_path.to_string_lossy().into_owned())
    );
    assert_eq!(
        command_env_value(&command, "WINEPREFIX"),
        Some(compatdata_path.join("pfx").to_string_lossy().into_owned())
    );
    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_CLIENT_INSTALL_PATH"),
        Some(steam_client_path.to_string_lossy().into_owned())
    );
}

#[test]
fn proton_trainer_command_stages_support_files_into_prefix() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("pfx");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("sample.exe");
    let trainer_config_path = trainer_source_dir.join("sample.ini");
    let proton_path = temp_dir.path().join("proton");
    let log_path = temp_dir.path().join("trainer.log");

    fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
    fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    fs::write(&trainer_path, b"trainer").expect("trainer exe");
    fs::write(&trainer_config_path, b"config").expect("trainer config");
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
        trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
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
        args,
        vec![
            "run".to_string(),
            "C:\\CrossHook\\StagedTrainers\\sample\\sample.exe".to_string(),
        ]
    );
    assert!(prefix_path
        .join("drive_c/CrossHook/StagedTrainers/sample/sample.exe")
        .exists());
    assert!(prefix_path
        .join("drive_c/CrossHook/StagedTrainers/sample/sample.ini")
        .exists());
}

#[test]
fn proton_trainer_command_uses_source_directory_without_staging() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("sample.exe");
    let log_path = temp_dir.path().join("trainer.log");

    fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
    fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    fs::write(&trainer_path, b"trainer").expect("trainer exe");
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
        args,
        vec![
            "run".to_string(),
            trainer_path.to_string_lossy().into_owned()
        ]
    );
    assert_eq!(
        command
            .as_std()
            .get_current_dir()
            .map(|path| path.to_string_lossy().into_owned()),
        Some(trainer_source_dir.to_string_lossy().into_owned())
    );
    assert!(!prefix_path
        .join("drive_c/CrossHook/StagedTrainers/sample")
        .exists());
}

#[test]
fn proton_trainer_command_uses_pfx_child_when_prefix_path_is_compatdata_root() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let compatdata_root = temp_dir.path().join("compatdata-root");
    let wine_prefix_path = compatdata_root.join("pfx");
    let trainer_source_dir = temp_dir.path().join("trainer");
    let trainer_path = trainer_source_dir.join("aurora.exe");
    let proton_path = temp_dir.path().join("proton");
    let log_path = temp_dir.path().join("trainer.log");

    fs::create_dir_all(wine_prefix_path.join("drive_c")).expect("prefix dir");
    fs::create_dir_all(&trainer_source_dir).expect("trainer source dir");
    fs::write(&trainer_path, b"trainer").expect("trainer exe");
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
        trainer_loading_mode: crate::profile::TrainerLoadingMode::CopyToPrefix,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: compatdata_root.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
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
    let envs = command
        .as_std()
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|inner| inner.to_string_lossy().into_owned()),
            )
        })
        .collect::<Vec<_>>();

    assert!(wine_prefix_path
        .join("drive_c/CrossHook/StagedTrainers/aurora/aurora.exe")
        .exists());
    assert!(envs.iter().any(|(key, value)| {
        key == "WINEPREFIX" && value.as_deref() == Some(wine_prefix_path.to_string_lossy().as_ref())
    }));
    assert!(envs.iter().any(|(key, value)| {
        key == "STEAM_COMPAT_DATA_PATH"
            && value.as_deref() == Some(compatdata_root.to_string_lossy().as_ref())
    }));
}
