use super::support::{command_env_value, steam_request};
use crate::launch::script_runner::{build_helper_command, build_trainer_command};
use std::collections::BTreeMap;

#[test]
fn helper_command_includes_expected_script_arguments() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("script.sh");
    let log_path = temp_dir.path().join("log.txt");
    let request = steam_request();

    let command = build_helper_command(&request, &script_path, &log_path).expect("helper command");
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
            "--trainer-loading-mode".to_string(),
            "source_directory".to_string(),
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
fn helper_command_includes_working_directory_when_configured() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-helper.sh");
    let log_path = temp_dir.path().join("helper.log");
    let mut request = steam_request();
    request.runtime.working_directory = "/games/example".to_string();

    let command = build_helper_command(&request, &script_path, &log_path).expect("helper command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.contains(&"--working-directory".to_string()));
    assert!(args.contains(&"/games/example".to_string()));
}

#[test]
fn trainer_command_includes_steam_app_id_and_trainer_arguments() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-trainer.sh");
    let log_path = temp_dir.path().join("trainer.log");
    let request = steam_request();

    let command =
        build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

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
            "--steam-app-id".to_string(),
            "12345".to_string(),
            "--trainer-path".to_string(),
            "/trainers/trainer.exe".to_string(),
            "--trainer-host-path".to_string(),
            "/trainers/trainer.exe".to_string(),
            "--trainer-loading-mode".to_string(),
            "source_directory".to_string(),
            "--log-file".to_string(),
            log_path.to_string_lossy().into_owned(),
        ]
    );
}

#[test]
fn trainer_command_preserves_custom_env_while_ignoring_launch_optimization_env() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-trainer.sh");
    let log_path = temp_dir.path().join("trainer.log");
    let mut request = steam_request();
    request.optimizations.enabled_option_ids = vec![
        "disable_steam_input".to_string(),
        "enable_dxvk_async".to_string(),
    ];
    request.custom_env_vars =
        BTreeMap::from([("DRI_PRIME".to_string(), "pci-0000_0a_00_0!".to_string())]);

    let command =
        build_trainer_command(&request, &script_path, &log_path).expect("trainer command");

    assert_eq!(command_env_value(&command, "PROTON_NO_STEAMINPUT"), None);
    assert_eq!(command_env_value(&command, "DXVK_ASYNC"), None);
    assert_eq!(
        command_env_value(&command, "DRI_PRIME"),
        Some("pci-0000_0a_00_0!".to_string())
    );
    assert_eq!(
        command_env_value(&command, "CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS"),
        None
    );
    assert_eq!(
        command_env_value(&command, "CROSSHOOK_TRAINER_CUSTOM_ENV_KEYS"),
        Some("DRI_PRIME".to_string())
    );
    assert_eq!(
        command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
        Some("/tmp/compat".to_string())
    );
}

#[test]
fn helper_command_includes_trainer_gamescope_when_configured() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-helper.sh");
    let log_path = temp_dir.path().join("helper.log");
    let mut request = steam_request();
    request.launch_game_only = false;
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
        enabled: true,
        allow_nested: true,
        fullscreen: true,
        ..Default::default()
    });

    let command = build_helper_command(&request, &script_path, &log_path).expect("helper command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.contains(&"--gamescope-enabled".to_string()));
    assert!(args.contains(&"--gamescope-allow-nested".to_string()));
    assert!(args.contains(&"--gamescope-arg".to_string()));
    assert!(args.contains(&"-f".to_string()));
}

#[test]
fn trainer_command_includes_working_directory_when_configured() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let script_path = temp_dir.path().join("steam-launch-trainer.sh");
    let log_path = temp_dir.path().join("trainer.log");
    let mut request = steam_request();
    request.runtime.working_directory = "/games/example".to_string();

    let command =
        build_trainer_command(&request, &script_path, &log_path).expect("trainer command");
    let args = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.contains(&"--working-directory".to_string()));
    assert!(args.contains(&"/games/example".to_string()));
}

/// The Steam helper scripts must refuse a missing, symlinked or non-regular
/// log file before writing to it. Each script's guard line is run on its own,
/// with `fail` stubbed to exit 3.
#[test]
fn helper_script_log_guards_refuse_missing_symlinked_and_non_regular_files() {
    let helpers = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-helpers");
    for script in [
        "steam-launch-helper.sh",
        "steam-launch-trainer.sh",
        "steam-host-trainer-runner.sh",
    ] {
        let text = std::fs::read_to_string(helpers.join(script)).expect("read helper script");
        let guard = text
            .lines()
            .find(|line| line.contains("Log file missing or unsafe."))
            .unwrap_or_else(|| panic!("{script} has no log guard"));
        let run = |log_file: &std::path::Path| {
            std::process::Command::new("bash")
                .arg("-c")
                .arg(format!(
                    "fail() {{ exit 3; }}; log_file=\"$1\"; {guard}; exit 0"
                ))
                .arg("bash")
                .arg(log_file)
                .status()
                .expect("run guard")
                .code()
        };

        let dir = tempfile::tempdir().expect("temp dir");
        let log = dir.path().join("run.log");
        assert_eq!(run(&log), Some(3), "{script}: missing file");

        std::fs::write(&log, "").expect("create log");
        assert_eq!(run(&log), Some(0), "{script}: regular owned file");

        let link = dir.path().join("link.log");
        std::os::unix::fs::symlink(&log, &link).expect("symlink");
        assert_eq!(run(&link), Some(3), "{script}: symlink");

        assert_eq!(run(dir.path()), Some(3), "{script}: directory");
    }
}
