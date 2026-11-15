//! Tests for the production entry points in `runtime_files/mod.rs`.

use std::path::PathBuf;

use super::super::{
    launch_log_dir_candidates_from, launch_log_dir_from, RuntimeDirError, RuntimeDirInputs,
    FLATPAK_APP_ID,
};

#[test]
fn unusable_runtime_root_falls_back_to_the_state_candidate() {
    let state = tempfile::tempdir().unwrap();
    let inputs = RuntimeDirInputs {
        runtime_dir: Some(state.path().join("missing-runtime")),
        state_dir: Some(state.path().to_path_buf()),
        flatpak: false,
    };
    let dir = launch_log_dir_from(&inputs).unwrap();
    assert_eq!(dir.path(), state.path().join("crosshook").join("logs"));
    assert!(dir.path().is_dir());
}

#[test]
fn all_candidates_unusable_yields_no_usable_dir() {
    let scratch = tempfile::tempdir().unwrap();
    let inputs = RuntimeDirInputs {
        runtime_dir: Some(scratch.path().join("missing-runtime")),
        state_dir: Some(PathBuf::from("relative/state")),
        flatpak: false,
    };
    assert!(matches!(
        launch_log_dir_from(&inputs),
        Err(RuntimeDirError::NoUsableDir)
    ));
}

#[test]
fn no_inputs_yields_no_usable_dir() {
    let inputs = RuntimeDirInputs {
        runtime_dir: None,
        state_dir: None,
        flatpak: false,
    };
    assert!(matches!(
        launch_log_dir_from(&inputs),
        Err(RuntimeDirError::NoUsableDir)
    ));
}

#[test]
fn native_candidates_also_list_the_flatpak_path_and_never_tmp() {
    let inputs = RuntimeDirInputs {
        runtime_dir: Some(PathBuf::from("/run/user/1000")),
        state_dir: Some(PathBuf::from("/home/u/.local/state")),
        flatpak: false,
    };
    let paths = launch_log_dir_candidates_from(&inputs);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/run/user/1000/crosshook/logs"),
            PathBuf::from("/home/u/.local/state/crosshook/logs"),
            PathBuf::from(format!("/run/user/1000/app/{FLATPAK_APP_ID}/logs")),
        ]
    );
    assert!(paths.iter().all(|path| !path.starts_with("/tmp")));
}

#[test]
fn flatpak_candidates_are_not_duplicated() {
    let inputs = RuntimeDirInputs {
        runtime_dir: Some(PathBuf::from("/run/user/1000")),
        state_dir: None,
        flatpak: true,
    };
    let paths = launch_log_dir_candidates_from(&inputs);
    assert_eq!(
        paths,
        vec![PathBuf::from(format!(
            "/run/user/1000/app/{FLATPAK_APP_ID}/logs"
        )),]
    );
}
