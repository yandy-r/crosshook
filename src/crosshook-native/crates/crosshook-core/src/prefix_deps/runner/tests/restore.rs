use std::fs;
use std::time::Duration;

use tempfile::tempdir;

use super::{make_fake_binary, make_install_restore_binary, write_registry_fixture};
use crate::prefix_deps::runner::{prepare_install_packages, restore_prefix_windows_version};
use crate::prefix_deps::{PrefixDepsTool, PrefixVersionRestorePlan, PrefixVersionRestoreState};

#[tokio::test]
async fn restoration_timeout_is_reported_separately() {
    let tmp = tempdir().unwrap();
    let captured_calls = tmp.path().join("calls.txt");
    let binary = make_install_restore_binary(tmp.path(), &captured_calls, 0, 0, 2, true);
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
    );

    let prepared = prepare_install_packages(
        &binary,
        tmp.path().to_str().unwrap(),
        &["dotnet48".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .unwrap();
    let child = prepared
        .spawn_with_restore_timeout(Duration::from_millis(50))
        .unwrap();
    let outcome = child.wait_and_restore().await;
    assert!(outcome.install_succeeded);
    assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Failed);
    assert!(outcome
        .restore_error
        .as_deref()
        .is_some_and(|error| error.contains("timed out")));
}

#[tokio::test]
async fn restoration_verification_mismatch_is_reported_as_failure() {
    let tmp = tempdir().unwrap();
    let captured_calls = tmp.path().join("calls.txt");
    let binary = make_install_restore_binary(tmp.path(), &captured_calls, 0, 0, 0, false);
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
    );

    let prepared = prepare_install_packages(
        &binary,
        tmp.path().to_str().unwrap(),
        &["dotnet48".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .unwrap();
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"7601\"\n\"ProductName\"=\"Microsoft Windows 7\"",
    );

    let outcome = prepared.spawn().unwrap().wait_and_restore().await;
    assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Failed);
    assert!(outcome
        .restore_error
        .as_deref()
        .is_some_and(|error| error.contains("verification")));
}

#[tokio::test]
async fn restart_restore_rejects_invalid_restore_verb_before_spawn() {
    let tmp = tempdir().unwrap();
    let captured_calls = tmp.path().join("calls.txt");
    let binary = make_fake_binary(
        tmp.path(),
        "winetricks",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
            captured_calls.display()
        ),
    );
    let pfx = tmp.path().join("pfx");
    fs::create_dir_all(&pfx).unwrap();
    let plan = PrefixVersionRestorePlan {
        binary_path: binary,
        prefix_path: pfx.to_string_lossy().into_owned(),
        tool_type: PrefixDepsTool::Winetricks,
        steam_app_id: None,
        restore_verb: "-q".to_string(),
    };

    let outcome = restore_prefix_windows_version(&plan).await;

    assert_eq!(outcome.state, PrefixVersionRestoreState::Failed);
    assert!(!captured_calls.exists(), "invalid restore must not spawn");
}
