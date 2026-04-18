#![cfg(test)]

use std::fs;
use std::path::Path;

use super::super::*;
use super::fixtures::*;
use crate::launch::request::{LaunchRequest, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::profile::TrainerLoadingMode;

#[test]
fn preview_runtime_environment_matches_proton_setup_for_compat_root() {
    let (_td, request) = proton_request();
    let preview = build_launch_preview(&request).expect("preview");
    let environment = preview.environment.expect("environment");
    let proton_setup = preview.proton_setup.expect("proton setup");

    let wine_prefix = environment
        .iter()
        .find(|variable| variable.key == "WINEPREFIX")
        .expect("WINEPREFIX");
    let compat_path = environment
        .iter()
        .find(|variable| variable.key == "STEAM_COMPAT_DATA_PATH")
        .expect("STEAM_COMPAT_DATA_PATH");

    assert_eq!(wine_prefix.value, proton_setup.wine_prefix_path);
    assert_eq!(compat_path.value, proton_setup.compat_data_path);
}

#[test]
fn preview_runtime_environment_matches_proton_setup_for_pfx_root() {
    let (_td, mut request) = proton_request();
    let prefix_path = Path::new(&request.runtime.prefix_path).join("pfx");
    fs::create_dir_all(&prefix_path).expect("create pfx dir");
    request.runtime.prefix_path = prefix_path.to_string_lossy().into_owned();

    let preview = build_launch_preview(&request).expect("preview");
    let environment = preview.environment.expect("environment");
    let proton_setup = preview.proton_setup.expect("proton setup");

    let wine_prefix = environment
        .iter()
        .find(|variable| variable.key == "WINEPREFIX")
        .expect("WINEPREFIX");
    let compat_path = environment
        .iter()
        .find(|variable| variable.key == "STEAM_COMPAT_DATA_PATH")
        .expect("STEAM_COMPAT_DATA_PATH");

    assert_eq!(wine_prefix.value, proton_setup.wine_prefix_path);
    assert_eq!(compat_path.value, proton_setup.compat_data_path);
    assert_eq!(
        compat_path.value,
        prefix_path
            .parent()
            .expect("compatdata parent")
            .to_string_lossy()
            .into_owned()
    );
}

#[test]
fn preview_proton_dxvk_custom_matches_runtime_command_env() {
    use std::collections::BTreeMap;

    let (_td, mut request) = proton_request();
    request.optimizations.enabled_option_ids = vec!["enable_dxvk_async".to_string()];
    request.custom_env_vars = BTreeMap::from([("DXVK_ASYNC".to_string(), "0".to_string())]);

    let preview = build_launch_preview(&request).expect("preview");
    let log_path = _td.path().join("parity.log");
    let command = crate::launch::script_runner::build_proton_game_command(&request, &log_path)
        .expect("command");

    let dxvk = preview
        .environment
        .as_ref()
        .expect("environment")
        .iter()
        .find(|v| v.key == "DXVK_ASYNC")
        .expect("DXVK_ASYNC in preview");
    assert_eq!(dxvk.value, "0");
    assert_eq!(dxvk.source, EnvVarSource::ProfileCustom);

    let cmd_val = command
        .as_std()
        .get_envs()
        .find_map(|(k, v)| {
            (k == std::ffi::OsStr::new("DXVK_ASYNC"))
                .then(|| v.map(|x| x.to_string_lossy().into_owned()))
        })
        .flatten();
    assert_eq!(cmd_val.as_deref(), Some("0"));
}

#[test]
fn preview_proton_verb_is_waitforexitandrun_for_game_and_runinprefix_for_trainer() {
    // Game launch: PROTON_VERB should be "waitforexitandrun"
    let (_td, request) = proton_request();
    let preview = build_launch_preview(&request).expect("preview");
    let env = preview.environment.expect("environment");
    let verb = env
        .iter()
        .find(|v| v.key == "PROTON_VERB")
        .expect("PROTON_VERB in game preview env");
    assert_eq!(verb.value, "waitforexitandrun");
    assert_eq!(verb.source, EnvVarSource::ProtonRuntime);

    // Trainer-only launch: PROTON_VERB should be "runinprefix"
    let (_td2, mut trainer_request) = proton_request();
    trainer_request.launch_trainer_only = true;
    trainer_request.launch_game_only = false;
    let trainer_preview = build_launch_preview(&trainer_request).expect("trainer preview");
    let trainer_env = trainer_preview.environment.expect("trainer environment");
    let trainer_verb = trainer_env
        .iter()
        .find(|v| v.key == "PROTON_VERB")
        .expect("PROTON_VERB in trainer preview env");
    assert_eq!(trainer_verb.value, "runinprefix");
    assert_eq!(trainer_verb.source, EnvVarSource::ProtonRuntime);
}

#[test]
fn preview_runtime_proton_env_includes_pressure_vessel_paths() {
    let (_td, mut request) = proton_request();
    let shared_root = Path::new(&request.game_path)
        .parent()
        .expect("game parent")
        .join("pressure-vessel");
    let game_dir = shared_root.join("game");
    let trainer_dir = shared_root.join("trainer");
    let working_dir = shared_root.join("working");

    fs::create_dir_all(&game_dir).expect("game dir");
    fs::create_dir_all(&trainer_dir).expect("trainer dir");
    fs::create_dir_all(&working_dir).expect("working dir");

    request.game_path = game_dir.join("game.exe").to_string_lossy().into_owned();
    request.trainer_host_path = trainer_dir
        .join("trainer.exe")
        .to_string_lossy()
        .into_owned();
    request.runtime.working_directory = working_dir.to_string_lossy().into_owned();
    fs::write(&request.game_path, b"game").expect("game exe");
    fs::write(&request.trainer_host_path, b"trainer").expect("trainer exe");

    let preview = build_launch_preview(&request).expect("preview");
    let env = preview.environment.expect("environment");
    let expected_paths = format!(
        "{}:{}:{}",
        game_dir.to_string_lossy(),
        trainer_dir.to_string_lossy(),
        working_dir.to_string_lossy()
    );

    let steam_compat_library_paths = env
        .iter()
        .find(|var| var.key == "STEAM_COMPAT_LIBRARY_PATHS")
        .expect("STEAM_COMPAT_LIBRARY_PATHS in preview env");
    assert_eq!(steam_compat_library_paths.value, expected_paths);
    assert_eq!(
        steam_compat_library_paths.source,
        EnvVarSource::ProtonRuntime
    );

    let pressure_vessel_filesystems_rw = env
        .iter()
        .find(|var| var.key == "PRESSURE_VESSEL_FILESYSTEMS_RW")
        .expect("PRESSURE_VESSEL_FILESYSTEMS_RW in preview env");
    assert_eq!(pressure_vessel_filesystems_rw.value, expected_paths);
    assert_eq!(
        pressure_vessel_filesystems_rw.source,
        EnvVarSource::ProtonRuntime
    );
}

#[test]
fn preview_runtime_proton_env_pressure_vessel_omits_trainer_under_copy_to_prefix() {
    let (_td, mut request) = proton_request();
    let shared_root = Path::new(&request.game_path)
        .parent()
        .expect("game parent")
        .join("pressure-vessel-copy");
    let game_dir = shared_root.join("game");
    let trainer_dir = shared_root.join("trainer");
    let working_dir = shared_root.join("working");

    fs::create_dir_all(&game_dir).expect("game dir");
    fs::create_dir_all(&trainer_dir).expect("trainer dir");
    fs::create_dir_all(&working_dir).expect("working dir");

    request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;
    request.game_path = game_dir.join("game.exe").to_string_lossy().into_owned();
    request.trainer_host_path = trainer_dir
        .join("trainer.exe")
        .to_string_lossy()
        .into_owned();
    request.runtime.working_directory = working_dir.to_string_lossy().into_owned();
    fs::write(&request.game_path, b"game").expect("game exe");
    fs::write(&request.trainer_host_path, b"trainer").expect("trainer exe");

    let preview = build_launch_preview(&request).expect("preview");
    let env = preview.environment.expect("environment");
    let expected_paths = format!(
        "{}:{}",
        game_dir.to_string_lossy(),
        working_dir.to_string_lossy()
    );

    let steam_compat_library_paths = env
        .iter()
        .find(|var| var.key == "STEAM_COMPAT_LIBRARY_PATHS")
        .expect("STEAM_COMPAT_LIBRARY_PATHS in preview env");
    assert_eq!(steam_compat_library_paths.value, expected_paths);
    assert!(
        !steam_compat_library_paths
            .value
            .contains(trainer_dir.to_string_lossy().as_ref()),
        "copy_to_prefix should omit trainer dir: {}",
        steam_compat_library_paths.value
    );

    let pressure_vessel_filesystems_rw = env
        .iter()
        .find(|var| var.key == "PRESSURE_VESSEL_FILESYSTEMS_RW")
        .expect("PRESSURE_VESSEL_FILESYSTEMS_RW in preview env");
    assert_eq!(pressure_vessel_filesystems_rw.value, expected_paths);
    assert!(
        !pressure_vessel_filesystems_rw
            .value
            .contains(trainer_dir.to_string_lossy().as_ref()),
        "copy_to_prefix should omit trainer dir: {}",
        pressure_vessel_filesystems_rw.value
    );
}

#[test]
fn preview_pushes_protonpath_env_when_use_umu() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    let mut request = LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: crate::settings::UmuPreference::Umu,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let preview = build_launch_preview(&request).unwrap();
    let env = preview.environment.unwrap();
    let protonpath = env
        .iter()
        .find(|e| e.key == "PROTONPATH")
        .expect("expected PROTONPATH env entry");
    assert_eq!(protonpath.value, "/opt/proton/GE-Proton9-20");
    assert!(matches!(protonpath.source, EnvVarSource::ProtonRuntime));
}

#[test]
fn preview_steam_branch_does_not_push_protonpath() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    let mut request = LaunchRequest {
        method: METHOD_STEAM_APPLAUNCH.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: crate::settings::UmuPreference::Umu,
        ..Default::default()
    };
    request.steam.app_id = "70".to_string();
    request.steam.compatdata_path = "/tmp/compat".to_string();
    request.steam.proton_path = "/opt/steam/proton/proton".to_string();

    let preview = build_launch_preview(&request).unwrap();
    let env = preview.environment.unwrap();
    assert!(
        env.iter().find(|e| e.key == "PROTONPATH").is_none(),
        "Steam branch must not push PROTONPATH"
    );
    assert!(
        preview.proton_setup.unwrap().umu_run_path.is_none(),
        "Steam ProtonSetup.umu_run_path must be None"
    );
}
