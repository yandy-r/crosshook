use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use super::LaunchRequest;

const BASH_EXECUTABLE: &str = "/bin/bash";
const DEFAULT_PATH: &str = "/usr/bin:/bin";
const DEFAULT_SHELL: &str = "/bin/bash";
const DEFAULT_GAME_STARTUP_DELAY_SECONDS: &str = "30";
const DEFAULT_GAME_TIMEOUT_SECONDS: &str = "90";
const DEFAULT_TRAINER_TIMEOUT_SECONDS: &str = "10";
const STAGED_TRAINER_ROOT: &str = "CrossHook/StagedTrainers";
const SUPPORT_DIRECTORIES: [&str; 9] = [
    "assets",
    "data",
    "lib",
    "bin",
    "runtimes",
    "plugins",
    "locales",
    "cef",
    "resources",
];
const SHARED_DEPENDENCY_EXTENSIONS: [&str; 7] =
    ["dll", "json", "config", "ini", "pak", "dat", "bin"];

pub fn build_helper_command(
    request: &LaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> Command {
    let mut command = build_base_command(script_path);
    apply_host_environment(&mut command);
    apply_steam_proton_environment(&mut command, request);
    command.args(helper_arguments(request, log_path));
    command
}

pub fn build_trainer_command(
    request: &LaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> Command {
    let mut command = build_base_command(script_path);
    apply_host_environment(&mut command);
    apply_steam_proton_environment(&mut command, request);
    command.args(trainer_arguments(request, log_path));
    command
}

pub fn build_proton_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let mut command = Command::new(request.runtime.proton_path.trim());
    command.arg("run");
    command.arg(request.game_path.trim());
    command.env_clear();
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(&mut command, request);
    apply_working_directory(
        &mut command,
        request.runtime.working_directory.trim(),
        Path::new(request.game_path.trim()),
    );
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}

pub fn build_proton_trainer_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let staged_trainer_path = stage_trainer_into_prefix(
        Path::new(request.runtime.prefix_path.trim()),
        Path::new(request.trainer_host_path.trim()),
    )?;

    let mut command = Command::new(request.runtime.proton_path.trim());
    command.arg("run");
    command.arg(staged_trainer_path);
    command.env_clear();
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(&mut command, request);
    apply_working_directory(
        &mut command,
        request.runtime.working_directory.trim(),
        Path::new(request.trainer_host_path.trim()),
    );
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}

pub fn build_native_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let mut command = Command::new(request.game_path.trim());
    command.env_clear();
    apply_host_environment(&mut command);
    apply_working_directory(
        &mut command,
        request.runtime.working_directory.trim(),
        Path::new(request.game_path.trim()),
    );
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}

fn build_base_command(script_path: &Path) -> Command {
    let mut command = Command::new(BASH_EXECUTABLE);
    command.arg(script_path);
    command.env_clear();
    command
}

fn apply_host_environment(command: &mut Command) {
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
}

fn apply_steam_proton_environment(command: &mut Command, request: &LaunchRequest) {
    set_env(
        command,
        "STEAM_COMPAT_DATA_PATH",
        request.steam.compatdata_path.trim(),
    );
    set_env(
        command,
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        request.steam.steam_client_install_path.trim(),
    );
    set_env(
        command,
        "WINEPREFIX",
        &Path::new(request.steam.compatdata_path.trim())
            .join("pfx")
            .to_string_lossy()
            .into_owned(),
    );
}

fn apply_runtime_proton_environment(command: &mut Command, request: &LaunchRequest) {
    let prefix_path = request.runtime.prefix_path.trim();
    set_env(command, "WINEPREFIX", prefix_path);

    let prefix = Path::new(prefix_path);
    let compat_data_path = if prefix.file_name().and_then(|value| value.to_str()) == Some("pfx") {
        prefix.parent().unwrap_or(prefix)
    } else {
        prefix
    };

    set_env(
        command,
        "STEAM_COMPAT_DATA_PATH",
        compat_data_path.to_string_lossy().as_ref(),
    );

    if let Some(steam_client_install_path) =
        resolve_steam_client_install_path(request.steam.steam_client_install_path.trim())
    {
        set_env(
            command,
            "STEAM_COMPAT_CLIENT_INSTALL_PATH",
            steam_client_install_path.as_str(),
        );
    }
}

fn resolve_steam_client_install_path(configured_path: &str) -> Option<String> {
    let trimmed_configured_path = configured_path.trim();
    if !trimmed_configured_path.is_empty() {
        return Some(trimmed_configured_path.to_string());
    }

    if let Ok(steam_client_install_path) = env::var("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        let trimmed = steam_client_install_path.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let home_path = env::var_os("HOME").map(PathBuf::from)?;
    for candidate in [
        home_path.join(".local/share/Steam"),
        home_path.join(".steam/root"),
        home_path.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ] {
        if candidate.join("steamapps").is_dir() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

fn env_value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn set_env(command: &mut Command, key: &str, value: impl AsRef<str>) {
    command.env(key, value.as_ref());
}

fn helper_arguments(request: &LaunchRequest, log_path: &Path) -> Vec<OsString> {
    let mut arguments = vec![
        "--appid".to_string().into(),
        request.steam.app_id.clone().into(),
        "--compatdata".to_string().into(),
        request.steam.compatdata_path.clone().into(),
        "--proton".into(),
        request.steam.proton_path.clone().into(),
        "--steam-client".into(),
        request.steam.steam_client_install_path.clone().into(),
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

fn trainer_arguments(request: &LaunchRequest, log_path: &Path) -> Vec<OsString> {
    vec![
        "--compatdata".into(),
        request.steam.compatdata_path.clone().into(),
        "--proton".into(),
        request.steam.proton_path.clone().into(),
        "--steam-client".into(),
        request.steam.steam_client_install_path.clone().into(),
        "--trainer-path".into(),
        request.trainer_path.clone().into(),
        "--trainer-host-path".into(),
        request.trainer_host_path.clone().into(),
        "--log-file".into(),
        log_path.as_os_str().to_owned(),
    ]
}

fn attach_log_stdio(command: &mut Command, log_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    command.stdout(Stdio::from(stdout));
    command.stderr(Stdio::from(stderr));
    Ok(())
}

fn apply_working_directory(command: &mut Command, configured_directory: &str, primary_path: &Path) {
    if !configured_directory.is_empty() {
        command.current_dir(configured_directory);
        return;
    }

    if let Some(parent) = primary_path.parent() {
        if !parent.as_os_str().is_empty() {
            command.current_dir(parent);
        }
    }
}

fn stage_trainer_into_prefix(
    prefix_path: &Path,
    trainer_host_path: &Path,
) -> std::io::Result<String> {
    let trainer_file_name = trainer_host_path
        .file_name()
        .ok_or_else(|| io_error("trainer host path is missing a file name"))?;
    let trainer_base_name = trainer_host_path
        .file_stem()
        .ok_or_else(|| io_error("trainer host path is missing a file stem"))?;
    let trainer_source_dir = trainer_host_path
        .parent()
        .ok_or_else(|| io_error("trainer host path is missing a parent directory"))?;

    let staged_root = prefix_path
        .join("drive_c")
        .join(PathBuf::from(STAGED_TRAINER_ROOT));
    let staged_directory = staged_root.join(trainer_base_name);
    let staged_host_path = staged_directory.join(trainer_file_name);

    if staged_directory.exists() {
        fs::remove_dir_all(&staged_directory)?;
    }

    fs::create_dir_all(&staged_directory)?;
    fs::copy(trainer_host_path, &staged_host_path)?;
    stage_trainer_support_files(
        trainer_source_dir,
        &staged_directory,
        trainer_file_name,
        trainer_base_name.to_string_lossy().as_ref(),
    )?;

    Ok(format!(
        "C:\\CrossHook\\StagedTrainers\\{}\\{}",
        trainer_base_name.to_string_lossy(),
        trainer_file_name.to_string_lossy()
    ))
}

fn stage_trainer_support_files(
    trainer_source_dir: &Path,
    staged_target_dir: &Path,
    trainer_file_name: &std::ffi::OsStr,
    trainer_base_name: &str,
) -> std::io::Result<()> {
    for entry in fs::read_dir(trainer_source_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();

        if file_name == trainer_file_name {
            continue;
        }

        if path.is_file() && should_stage_support_file(&file_name, trainer_base_name) {
            fs::copy(&path, staged_target_dir.join(&file_name))?;
        }
    }

    for directory in SUPPORT_DIRECTORIES {
        let source = trainer_source_dir.join(directory);
        if source.is_dir() {
            copy_dir_all(&source, &staged_target_dir.join(directory))?;
        }
    }

    Ok(())
}

fn should_stage_support_file(file_name: &std::ffi::OsStr, trainer_base_name: &str) -> bool {
    let file_name = file_name.to_string_lossy();
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();

    if !SHARED_DEPENDENCY_EXTENSIONS.contains(&extension.as_str()) {
        return false;
    }

    if file_name.starts_with(&format!("{trainer_base_name}.")) {
        return true;
    }

    true
}

fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

fn io_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_executable_file(path: &Path) {
        fs::write(path, b"test").expect("write file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    fn steam_request() -> LaunchRequest {
        LaunchRequest {
            method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
            game_path: "/games/My Game/game.exe".to_string(),
            trainer_path: "/trainers/trainer.exe".to_string(),
            trainer_host_path: "/trainers/trainer.exe".to_string(),
            steam: crate::launch::SteamLaunchConfig {
                app_id: "12345".to_string(),
                compatdata_path: "/tmp/compat".to_string(),
                proton_path: "/tmp/proton".to_string(),
                steam_client_install_path: "/tmp/steam".to_string(),
            },
            runtime: crate::launch::RuntimeLaunchConfig::default(),
            launch_trainer_only: false,
            launch_game_only: true,
        }
    }

    #[test]
    fn helper_command_includes_expected_script_arguments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let script_path = temp_dir.path().join("script.sh");
        let log_path = temp_dir.path().join("log.txt");
        let request = steam_request();

        let command = build_helper_command(&request, &script_path, &log_path);

        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

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
            steam: crate::launch::SteamLaunchConfig::default(),
            runtime: crate::launch::RuntimeLaunchConfig {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: proton_path.to_string_lossy().into_owned(),
                working_directory: String::new(),
            },
            launch_trainer_only: true,
            launch_game_only: false,
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
    fn proton_game_command_sets_compat_data_path_for_standalone_prefixes() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("standalone-prefix");
        let proton_path = temp_dir.path().join("proton");
        let game_path = temp_dir.path().join("game.exe");
        let log_path = temp_dir.path().join("game.log");
        let steam_client_path = temp_dir.path().join("steam");

        fs::create_dir_all(prefix_path.join("drive_c")).expect("prefix dir");
        fs::create_dir_all(steam_client_path.join("steamapps")).expect("steam client dir");
        write_executable_file(&proton_path);
        fs::write(&game_path, b"game").expect("game exe");

        let request = LaunchRequest {
            method: crate::launch::METHOD_PROTON_RUN.to_string(),
            game_path: game_path.to_string_lossy().into_owned(),
            trainer_path: String::new(),
            trainer_host_path: String::new(),
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
            },
            launch_trainer_only: false,
            launch_game_only: true,
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
}
