use std::path::Path;

use super::support::command_env_value;
use crate::launch::script_runner::build_proton_game_command;
use crate::launch::LaunchRequest;

fn write_umu_stub(dir: &Path) {
    let umu_stub = dir.join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn proton_game_command_swaps_to_umu_run_when_umu_preferred() {
    let dir = tempfile::tempdir().unwrap();
    write_umu_stub(dir.path());
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Umu,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("test.log");
    let command = build_proton_game_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        program.ends_with("/umu-run"),
        "expected umu-run program, got {program}"
    );
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        !args.contains(&"run".to_string()),
        "umu-run must not receive \"run\" subcommand arg; args: {args:?}"
    );
    assert_eq!(
        command_env_value(&command, "PROTONPATH"),
        Some("/opt/proton/GE-Proton9-20".to_string())
    );
}

#[test]
fn proton_game_command_falls_back_to_proton_when_umu_preferred_but_missing_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Umu,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("test.log");
    let command = build_proton_game_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        !program.ends_with("umu-run"),
        "expected direct proton when umu-run absent, got {program}"
    );
    assert_eq!(
        command_env_value(&command, "PROTONPATH"),
        None,
        "PROTONPATH must not be set when falling back"
    );
}

#[test]
fn auto_preference_uses_umu_when_umu_run_present() {
    let dir = tempfile::tempdir().unwrap();
    write_umu_stub(dir.path());
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Auto,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("test.log");
    let command = build_proton_game_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        program.ends_with("/umu-run"),
        "expected umu-run program with Auto preference when present, got {program}"
    );
    assert_eq!(
        command_env_value(&command, "PROTON_VERB"),
        Some("waitforexitandrun".to_string())
    );
    assert_eq!(
        command_env_value(&command, "PROTONPATH"),
        Some("/opt/proton/GE-Proton9-20".to_string())
    );
}

#[test]
fn auto_preference_falls_back_to_proton_when_umu_run_missing() {
    let dir = tempfile::tempdir().unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Auto,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("test.log");
    let command = build_proton_game_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        !program.ends_with("umu-run"),
        "expected direct proton fallback with Auto when umu-run absent, got {program}"
    );
    assert_eq!(command_env_value(&command, "PROTONPATH"), None);
    assert_eq!(command_env_value(&command, "PROTON_VERB"), None);
}

#[test]
fn proton_preference_always_uses_direct_proton() {
    let dir = tempfile::tempdir().unwrap();
    write_umu_stub(dir.path());
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: UmuPreference::Proton,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("test.log");
    let command = build_proton_game_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(!program.ends_with("umu-run"));
}
