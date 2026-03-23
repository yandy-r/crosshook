use std::env;
use std::ffi::OsString;
use std::path::Path;

use tokio::process::Command;

use super::SteamLaunchRequest;

const BASH_EXECUTABLE: &str = "/bin/bash";
const DEFAULT_PATH: &str = "/usr/bin:/bin";
const DEFAULT_SHELL: &str = "/bin/bash";
const DEFAULT_GAME_STARTUP_DELAY_SECONDS: &str = "30";
const DEFAULT_GAME_TIMEOUT_SECONDS: &str = "90";
const DEFAULT_TRAINER_TIMEOUT_SECONDS: &str = "10";

pub fn build_helper_command(
    request: &SteamLaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> Command {
    let mut command = build_base_command(script_path, request);
    command.args(helper_arguments(request, log_path));
    command
}

pub fn build_trainer_command(
    request: &SteamLaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> Command {
    let mut command = build_base_command(script_path, request);
    command.args(trainer_arguments(request, log_path));
    command
}

fn build_base_command(script_path: &Path, request: &SteamLaunchRequest) -> Command {
    let mut command = Command::new(BASH_EXECUTABLE);
    command.arg(script_path);
    command.env_clear();
    apply_clean_environment(&mut command, request);
    command
}

fn apply_clean_environment(command: &mut Command, request: &SteamLaunchRequest) {
    set_env(command, "HOME", env_value("HOME", ""));
    set_env(command, "USER", env_value("USER", ""));
    set_env(command, "LOGNAME", env_value("LOGNAME", ""));
    set_env(command, "SHELL", env_value("SHELL", DEFAULT_SHELL));
    set_env(command, "PATH", env_value("PATH", DEFAULT_PATH));
    set_env(command, "DISPLAY", env_value("DISPLAY", ""));
    set_env(command, "WAYLAND_DISPLAY", env_value("WAYLAND_DISPLAY", ""));
    set_env(command, "XDG_RUNTIME_DIR", env_value("XDG_RUNTIME_DIR", ""));
    set_env(
        command,
        "DBUS_SESSION_BUS_ADDRESS",
        env_value("DBUS_SESSION_BUS_ADDRESS", ""),
    );
    set_env(
        command,
        "STEAM_COMPAT_DATA_PATH",
        &request.steam_compat_data_path,
    );
    set_env(
        command,
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        &request.steam_client_install_path,
    );
    set_env(command, "WINEPREFIX", compatdata_wineprefix(request));
}

fn env_value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn set_env(command: &mut Command, key: &str, value: impl AsRef<str>) {
    command.env(key, value.as_ref());
}

fn compatdata_wineprefix(request: &SteamLaunchRequest) -> String {
    Path::new(&request.steam_compat_data_path)
        .join("pfx")
        .to_string_lossy()
        .into_owned()
}

fn helper_arguments(request: &SteamLaunchRequest, log_path: &Path) -> Vec<OsString> {
    let mut arguments = vec![
        "--appid".to_string().into(),
        request.steam_app_id.clone().into(),
        "--compatdata".to_string().into(),
        request.steam_compat_data_path.clone().into(),
        "--proton".into(),
        request.steam_proton_path.clone().into(),
        "--steam-client".into(),
        request.steam_client_install_path.clone().into(),
        "--game-exe-name".into(),
        request.game_executable_name().into(),
        "--trainer-path".into(),
        request.trainer_path.clone().into(),
        "--trainer-host-path".into(),
        request.trainer_host_path.clone().into(),
        "--log-file".into(),
        log_path.as_os_str().to_owned(),
        "--game-startup-delay-seconds".into(),
        DEFAULT_GAME_STARTUP_DELAY_SECONDS.into(),
        "--game-timeout-seconds".into(),
        DEFAULT_GAME_TIMEOUT_SECONDS.into(),
        "--trainer-timeout-seconds".into(),
        DEFAULT_TRAINER_TIMEOUT_SECONDS.into(),
    ];

    if request.launch_trainer_only {
        arguments.push("--trainer-only".into());
    }

    if request.launch_game_only {
        arguments.push("--game-only".into());
    }

    arguments
}

fn trainer_arguments(request: &SteamLaunchRequest, log_path: &Path) -> Vec<OsString> {
    vec![
        "--compatdata".into(),
        request.steam_compat_data_path.clone().into(),
        "--proton".into(),
        request.steam_proton_path.clone().into(),
        "--steam-client".into(),
        request.steam_client_install_path.clone().into(),
        "--trainer-path".into(),
        request.trainer_path.clone().into(),
        "--trainer-host-path".into(),
        request.trainer_host_path.clone().into(),
        "--log-file".into(),
        log_path.as_os_str().to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_command_includes_expected_script_arguments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("script.sh");
        let log_path = temp_dir.path().join("log.txt");
        let request = SteamLaunchRequest {
            game_path: "/games/My Game/game.exe".to_string(),
            trainer_path: "/trainers/trainer.exe".to_string(),
            trainer_host_path: "/trainers/trainer.exe".to_string(),
            steam_app_id: "12345".to_string(),
            steam_compat_data_path: "/tmp/compat".to_string(),
            steam_proton_path: "/tmp/proton".to_string(),
            steam_client_install_path: "/tmp/steam".to_string(),
            launch_trainer_only: false,
            launch_game_only: true,
        };

        let _command = build_helper_command(&request, &script_path, &log_path);

        let mut args = vec![script_path.to_string_lossy().into_owned()];
        args.extend(
            helper_arguments(&request, &log_path)
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned()),
        );

        assert_eq!(
            args,
            vec![
                script_path.to_string_lossy().into_owned(),
                "--appid".to_string(),
                "12345".to_string(),
                "--compatdata".to_string(),
                "/tmp/compat".to_string(),
                "--proton".to_string(),
                "/tmp/proton".to_string(),
                "--steam-client".to_string(),
                "/tmp/steam".to_string(),
                "--game-exe-name".to_string(),
                "game.exe".to_string(),
                "--trainer-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--trainer-host-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--log-file".to_string(),
                log_path.to_string_lossy().into_owned(),
                "--game-startup-delay-seconds".to_string(),
                "30".to_string(),
                "--game-timeout-seconds".to_string(),
                "90".to_string(),
                "--trainer-timeout-seconds".to_string(),
                "10".to_string(),
                "--game-only".to_string(),
            ]
        );
    }

    #[test]
    fn trainer_command_includes_expected_script_arguments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("script.sh");
        let log_path = temp_dir.path().join("log.txt");
        let request = SteamLaunchRequest {
            game_path: "/games/My Game/game.exe".to_string(),
            trainer_path: "/trainers/trainer.exe".to_string(),
            trainer_host_path: "/trainers/trainer.exe".to_string(),
            steam_app_id: "12345".to_string(),
            steam_compat_data_path: "/tmp/compat".to_string(),
            steam_proton_path: "/tmp/proton".to_string(),
            steam_client_install_path: "/tmp/steam".to_string(),
            launch_trainer_only: true,
            launch_game_only: false,
        };

        let _command = build_trainer_command(&request, &script_path, &log_path);

        let mut args = vec![script_path.to_string_lossy().into_owned()];
        args.extend(
            trainer_arguments(&request, &log_path)
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned()),
        );

        assert_eq!(
            args,
            vec![
                script_path.to_string_lossy().into_owned(),
                "--compatdata".to_string(),
                "/tmp/compat".to_string(),
                "--proton".to_string(),
                "/tmp/proton".to_string(),
                "--steam-client".to_string(),
                "/tmp/steam".to_string(),
                "--trainer-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--trainer-host-path".to_string(),
                "/trainers/trainer.exe".to_string(),
                "--log-file".to_string(),
                log_path.to_string_lossy().into_owned(),
            ]
        );
    }

    #[test]
    fn request_uses_last_path_segment_for_executable_name() {
        let request = SteamLaunchRequest {
            game_path: r"Z:\Games\Test Game\game.exe".to_string(),
            ..SteamLaunchRequest::default()
        };

        assert_eq!(request.game_executable_name(), "game.exe");
    }
}
