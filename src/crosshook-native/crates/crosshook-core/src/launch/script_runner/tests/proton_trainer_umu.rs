use std::path::Path;

use super::support::command_env_value;
use crate::launch::script_runner::{
    build_flatpak_steam_trainer_command, build_proton_trainer_command,
};
use crate::launch::LaunchRequest;

fn write_umu_stub(dir: &Path) {
    let umu_stub = dir.join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn proton_trainer_command_swaps_to_umu_run_when_umu_preferred() {
    let dir = tempfile::tempdir().unwrap();
    write_umu_stub(dir.path());
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        trainer_path: "/tmp/trainer.exe".to_string(),
        trainer_host_path: "/tmp/trainer.exe".to_string(),
        umu_preference: UmuPreference::Umu,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("trainer.log");
    let command = build_proton_trainer_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        program.ends_with("/umu-run"),
        "expected umu-run program, got {program}"
    );
    assert_eq!(
        command_env_value(&command, "PROTON_VERB"),
        Some("runinprefix".to_string())
    );
    assert_eq!(
        command_env_value(&command, "PROTONPATH"),
        Some("/opt/proton/GE-Proton9-20".to_string())
    );
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        !args.contains(&"run".to_string()),
        "umu-run must not receive \"run\"; args: {args:?}"
    );
}

#[test]
fn proton_trainer_command_falls_back_to_proton_when_umu_preferred_but_missing() {
    let dir = tempfile::tempdir().unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        trainer_path: "/tmp/trainer.exe".to_string(),
        trainer_host_path: "/tmp/trainer.exe".to_string(),
        umu_preference: UmuPreference::Umu,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("trainer.log");
    let command = build_proton_trainer_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        !program.ends_with("umu-run"),
        "expected direct proton fallback, got {program}"
    );
    assert_eq!(command_env_value(&command, "PROTONPATH"), None);
}

#[test]
fn auto_preference_uses_umu_trainer_when_present() {
    let dir = tempfile::tempdir().unwrap();
    write_umu_stub(dir.path());
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        trainer_path: "/tmp/trainer.exe".to_string(),
        trainer_host_path: "/tmp/trainer.exe".to_string(),
        umu_preference: UmuPreference::Auto,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("trainer.log");
    let command = build_proton_trainer_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        program.ends_with("/umu-run"),
        "expected umu-run for trainer with Auto preference when present, got {program}"
    );
    assert_eq!(
        command_env_value(&command, "PROTON_VERB"),
        Some("runinprefix".to_string())
    );
    assert_eq!(
        command_env_value(&command, "PROTONPATH"),
        Some("/opt/proton/GE-Proton9-20".to_string())
    );
}

#[test]
fn auto_preference_trainer_falls_back_to_proton_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_PROTON_RUN.to_string(),
        trainer_path: "/tmp/trainer.exe".to_string(),
        trainer_host_path: "/tmp/trainer.exe".to_string(),
        umu_preference: UmuPreference::Auto,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("trainer.log");
    let command = build_proton_trainer_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        !program.ends_with("umu-run"),
        "expected direct proton fallback for trainer with Auto when umu-run absent, got {program}"
    );
    assert_eq!(command_env_value(&command, "PROTON_VERB"), None);
    assert_eq!(command_env_value(&command, "PROTONPATH"), None);
}

#[test]
fn flatpak_steam_trainer_command_never_uses_umu_even_when_preferred() {
    let dir = tempfile::tempdir().unwrap();
    write_umu_stub(dir.path());
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    use crate::settings::UmuPreference;
    let mut request = LaunchRequest {
        method: crate::launch::METHOD_STEAM_APPLAUNCH.to_string(),
        trainer_path: "/tmp/trainer.exe".to_string(),
        trainer_host_path: "/tmp/trainer.exe".to_string(),
        umu_preference: UmuPreference::Umu,
        ..Default::default()
    };
    request.steam.app_id = "70".to_string();
    request.steam.compatdata_path = "/tmp/compat".to_string();
    request.steam.proton_path = "/opt/steam/proton/proton".to_string();

    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("steam-trainer.log");
    let command = build_flatpak_steam_trainer_command(&request, &log_path).unwrap();

    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    assert!(
        !program.ends_with("umu-run"),
        "Steam-context trainers must never use umu-run; got {program}"
    );
    assert_eq!(
        command_env_value(&command, "PROTONPATH"),
        None,
        "PROTONPATH must not be set under Steam opt-out"
    );
}
