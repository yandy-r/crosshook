use std::fs;
use std::path::Path;

use tokio::process::Command;

use crate::launch::LaunchRequest;

pub(super) fn write_executable_file(path: &Path) {
    write_executable_file_with_contents(path, b"test");
}

pub(super) fn write_executable_file_with_contents(path: &Path, contents: &[u8]) {
    fs::write(path, contents).expect("write file");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }
}

pub(super) fn steam_request() -> LaunchRequest {
    LaunchRequest {
        method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
        game_path: "/games/My Game/game.exe".to_string(),
        trainer_path: "/trainers/trainer.exe".to_string(),
        trainer_host_path: "/trainers/trainer.exe".to_string(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        steam: crate::launch::SteamLaunchConfig {
            app_id: "12345".to_string(),
            compatdata_path: "/tmp/compat".to_string(),
            proton_path: "/tmp/proton".to_string(),
            steam_client_install_path: "/tmp/steam".to_string(),
        },
        runtime: crate::launch::RuntimeLaunchConfig::default(),
        optimizations: crate::launch::request::LaunchOptimizationsRequest::default(),
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
        ..Default::default()
    }
}

pub(super) fn command_env_value(command: &Command, key: &str) -> Option<String> {
    command
        .as_std()
        .get_envs()
        .find_map(|(env_key, env_value)| {
            (env_key == std::ffi::OsStr::new(key))
                .then(|| env_value.map(|value| value.to_string_lossy().into_owned()))
        })
        .flatten()
}
