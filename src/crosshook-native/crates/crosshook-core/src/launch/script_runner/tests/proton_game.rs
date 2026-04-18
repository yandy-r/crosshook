use std::collections::BTreeMap;
use std::fs;

use super::support::{command_env_value, write_executable_file};
use crate::launch::script_runner::{build_native_game_command, build_proton_game_command};
use crate::launch::LaunchRequest;

#[test]
fn proton_game_command_applies_optimization_wrappers_and_env() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let wrapper_dir = temp_dir.path().join("wrappers");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let game_path = temp_dir.path().join("game.exe");
    let log_path = temp_dir.path().join("game.log");
    let steam_client_path = temp_dir.path().join("steam-client");
    let workspace_dir = prefix_path.join("workspace");

    fs::create_dir_all(&wrapper_dir).expect("wrapper dir");
    fs::create_dir_all(&prefix_path).expect("prefix dir");
    fs::create_dir_all(&workspace_dir).expect("workspace dir");
    fs::create_dir_all(&steam_client_path).expect("steam client dir");
    write_executable_file(&wrapper_dir.join("mangohud"));
    write_executable_file(&wrapper_dir.join("gamemoderun"));
    write_executable_file(&proton_path);
    fs::write(&game_path, b"game").expect("game exe");

    let _command_search_path =
        crate::launch::test_support::ScopedCommandSearchPath::new(&wrapper_dir);
    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: game_path.to_string_lossy().into_owned(),
        trainer_path: String::new(),
        trainer_host_path: String::new(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: String::new(),
            compatdata_path: String::new(),
            proton_path: String::new(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: workspace_dir.to_string_lossy().into_owned(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest {
            enabled_option_ids: vec![
                "show_mangohud_overlay".to_string(),
                "use_gamemode".to_string(),
                "disable_steam_input".to_string(),
            ],
        },
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_game_command(&request, &log_path).expect("game command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(command.as_std().get_program().to_string_lossy(), "mangohud");
    assert_eq!(
        args,
        vec![
            "gamemoderun".to_string(),
            proton_path.to_string_lossy().into_owned(),
            "run".to_string(),
            game_path.to_string_lossy().into_owned(),
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
        command_env_value(&command, "PROTON_NO_STEAMINPUT"),
        Some("1".to_string())
    );
    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
        Some(prefix_path.to_string_lossy().into_owned())
    );
    assert_eq!(
        command_env_value(&command, "WINEPREFIX"),
        Some(prefix_path.to_string_lossy().into_owned())
    );
    assert_eq!(command_env_value(&command, "GAMEID"), None);
}

#[test]
fn proton_game_custom_env_overrides_duplicate_optimization_key() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let game_path = temp_dir.path().join("game.exe");
    let log_path = temp_dir.path().join("game.log");
    let steam_client_path = temp_dir.path().join("steam-client");
    let workspace_dir = prefix_path.join("workspace");

    fs::create_dir_all(&prefix_path).expect("prefix dir");
    fs::create_dir_all(&workspace_dir).expect("workspace dir");
    fs::create_dir_all(&steam_client_path).expect("steam client dir");
    write_executable_file(&proton_path);
    fs::write(&game_path, b"game").expect("game exe");

    let _command_search_path =
        crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());
    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: game_path.to_string_lossy().into_owned(),
        trainer_path: String::new(),
        trainer_host_path: String::new(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: String::new(),
            compatdata_path: String::new(),
            proton_path: String::new(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: workspace_dir.to_string_lossy().into_owned(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest {
            enabled_option_ids: vec!["enable_dxvk_async".to_string()],
        },
        custom_env_vars: BTreeMap::from([("DXVK_ASYNC".to_string(), "0".to_string())]),
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_game_command(&request, &log_path).expect("game command");
    assert_eq!(
        command_env_value(&command, "DXVK_ASYNC"),
        Some("0".to_string())
    );
    assert_eq!(command_env_value(&command, "GAMEID"), None);
}

#[test]
fn proton_game_command_sets_proton_verb_to_waitforexitandrun() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let game_path = temp_dir.path().join("game.exe");
    let log_path = temp_dir.path().join("game.log");
    let steam_client_path = temp_dir.path().join("steam-client");
    let workspace_dir = prefix_path.join("workspace");

    fs::create_dir_all(&prefix_path).expect("prefix dir");
    fs::create_dir_all(&workspace_dir).expect("workspace dir");
    fs::create_dir_all(&steam_client_path).expect("steam client dir");
    write_executable_file(&proton_path);
    fs::write(&game_path, b"game").expect("game exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: game_path.to_string_lossy().into_owned(),
        trainer_path: String::new(),
        trainer_host_path: String::new(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: String::new(),
            compatdata_path: String::new(),
            proton_path: String::new(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: workspace_dir.to_string_lossy().into_owned(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_game_command(&request, &log_path).expect("game command");
    assert_eq!(command_env_value(&command, "PROTON_VERB"), None);
}

#[test]
fn proton_game_command_sets_pressure_vessel_paths_from_request() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let log_path = temp_dir.path().join("game.log");

    fs::create_dir_all(&prefix_path).expect("prefix dir");
    write_executable_file(&proton_path);

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/opt/games/TheGame/game.exe".to_string(),
        trainer_path: "/opt/trainers/trainer.exe".to_string(),
        trainer_host_path: "/opt/trainers/trainer.exe".to_string(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: "/srv/crosshook/workspaces/the-game".to_string(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_game_command(&request, &log_path).expect("game command");
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

#[test]
fn native_game_command_applies_custom_env_vars() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let game_path = temp_dir.path().join("run.sh");
    write_executable_file(&game_path);
    let log_path = temp_dir.path().join("native.log");

    let request = LaunchRequest {
        method: crate::launch::METHOD_NATIVE.to_string(),
        game_path: game_path.to_string_lossy().into_owned(),
        trainer_path: String::new(),
        trainer_host_path: String::new(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig::default(),
        runtime: crate::launch::RuntimeLaunchConfig::default(),
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        custom_env_vars: BTreeMap::from([("NATIVE_TEST_VAR".to_string(), "hello".to_string())]),
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        ..Default::default()
    };

    let command = build_native_game_command(&request, &log_path).expect("native command");
    assert_eq!(
        command_env_value(&command, "NATIVE_TEST_VAR"),
        Some("hello".to_string())
    );
}

#[test]
fn proton_game_command_sets_compat_data_path_for_standalone_prefixes() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("standalone-prefix");
    let proton_path = temp_dir.path().join("proton");
    let game_path = temp_dir.path().join("game.exe");
    let log_path = temp_dir.path().join("game.log");
    let steam_client_path = temp_dir.path().join("steam");

    fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
    fs::create_dir_all(steam_client_path.join("steamapps")).expect("steam client steamapps dir");
    fs::create_dir_all(steam_client_path.join("config")).expect("steam client config dir");
    write_executable_file(&proton_path);
    fs::write(&game_path, b"game").expect("game exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: game_path.to_string_lossy().into_owned(),
        trainer_path: String::new(),
        trainer_host_path: String::new(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: String::new(),
            compatdata_path: String::new(),
            proton_path: String::new(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_game_command(&request, &log_path).expect("game command");
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

    assert!(envs.iter().any(|(key, value)| {
        key == "STEAM_COMPAT_DATA_PATH"
            && value.as_deref() == Some(prefix_path.to_string_lossy().as_ref())
    }));
    assert!(envs.iter().any(|(key, value)| {
        key == "WINEPREFIX" && value.as_deref() == Some(prefix_path.to_string_lossy().as_ref())
    }));
    assert!(envs.iter().any(|(key, value)| {
        key == "STEAM_COMPAT_CLIENT_INSTALL_PATH"
            && value.as_deref() == Some(steam_client_path.to_string_lossy().as_ref())
    }));
}

#[test]
fn proton_game_command_does_not_include_unshare_net() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let prefix_path = temp_dir.path().join("prefix");
    let proton_path = temp_dir.path().join("proton");
    let game_path = temp_dir.path().join("game.exe");
    let log_path = temp_dir.path().join("game.log");
    let steam_client_path = temp_dir.path().join("steam-client");

    fs::create_dir_all(&prefix_path).expect("prefix dir");
    fs::create_dir_all(&steam_client_path).expect("steam client dir");
    write_executable_file(&proton_path);
    fs::write(&game_path, b"game").expect("game exe");

    let request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: game_path.to_string_lossy().into_owned(),
        trainer_path: String::new(),
        trainer_host_path: String::new(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: String::new(),
            compatdata_path: String::new(),
            proton_path: String::new(),
            steam_client_install_path: steam_client_path.to_string_lossy().into_owned(),
        },
        runtime: crate::launch::RuntimeLaunchConfig {
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        },
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: false,
        launch_game_only: true,
        network_isolation: true,
        profile_name: None,
        umu_preference: crate::settings::UmuPreference::Proton,
        ..Default::default()
    };

    let command = build_proton_game_command(&request, &log_path).expect("game command");
    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert_ne!(
        program, "unshare",
        "game command must NEVER use unshare --net"
    );
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        !args.contains(&"--net".to_string()),
        "game command args must not contain --net"
    );
}
