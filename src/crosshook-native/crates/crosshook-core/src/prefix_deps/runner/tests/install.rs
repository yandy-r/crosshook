use std::fs;
use std::path::Path;
use std::time::Duration;

use tempfile::tempdir;

use super::{make_fake_binary, make_install_restore_binary, write_registry_fixture};
use crate::prefix_deps::runner::{install_packages, prepare_install_packages};
use crate::prefix_deps::{PrefixDepsError, PrefixDepsTool, PrefixVersionRestoreState};

#[tokio::test]
async fn install_packages_rejects_invalid_verbs() {
    let tmp = tempdir().unwrap();
    let binary = make_fake_binary(tmp.path(), "winetricks", "#!/bin/sh\n");
    let pfx = tmp.path().join("pfx");
    fs::create_dir_all(&pfx).unwrap();

    let result = install_packages(
        &binary,
        tmp.path().to_str().unwrap(),
        &["-q".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, PrefixDepsError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

#[test]
fn install_packages_uses_winetricks_compatible_arguments() {
    let tmp = tempdir().unwrap();
    let binary = make_fake_binary(tmp.path(), "winetricks", "#!/bin/sh\nexit 0\n");
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

    let args = prepared
        .command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(args, vec!["-q", "dotnet48"]);
}

#[tokio::test]
async fn prepared_install_exposes_restore_plan_before_spawn() {
    let tmp = tempdir().unwrap();
    let captured_calls = tmp.path().join("calls.txt");
    let binary = make_install_restore_binary(tmp.path(), &captured_calls, 0, 0, 0, true);
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
    let plan = prepared.restore_plan().expect("restore plan");
    assert_eq!(plan.restore_verb, "win10");
    assert_eq!(plan.prefix_path, pfx.to_string_lossy());
    assert!(!captured_calls.exists(), "preparation must not spawn");

    let child = prepared.spawn().unwrap();
    let outcome = child.wait_and_restore().await;
    assert!(outcome.install_succeeded);
    assert_eq!(outcome.install_exit_code, Some(0));
    assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Succeeded);
    assert_eq!(outcome.restore_error, None);

    assert_eq!(
        fs::read_to_string(captured_calls).unwrap(),
        "-q dotnet48\n-q win10\n"
    );
}

#[tokio::test]
async fn failed_dependency_install_still_restores_prefix_windows_version() {
    let tmp = tempdir().unwrap();
    let captured_calls = tmp.path().join("calls.txt");
    let binary = make_install_restore_binary(tmp.path(), &captured_calls, 42, 0, 0, true);
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
    let outcome = prepared.spawn().unwrap().wait_and_restore().await;
    assert!(!outcome.install_succeeded);
    assert_eq!(outcome.install_exit_code, Some(42));
    assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Succeeded);

    assert_eq!(
        fs::read_to_string(captured_calls).unwrap(),
        "-q dotnet48\n-q win10\n"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn dropping_install_handle_kills_lifecycle_bound_process() {
    let tmp = tempdir().unwrap();
    let pid_file = tmp.path().join("pid.txt");
    let binary = make_fake_binary(
        tmp.path(),
        "winetricks",
        &format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec sleep 60\n",
            pid_file.display()
        ),
    );
    let pfx = tmp.path().join("pfx");
    fs::create_dir_all(&pfx).unwrap();

    let install = prepare_install_packages(
        &binary,
        tmp.path().to_str().unwrap(),
        &["win10".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .unwrap()
    .spawn()
    .unwrap();

    let pid = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(raw) = fs::read_to_string(&pid_file) {
                break raw.parse::<u32>().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("fake dependency process did not write its pid");

    drop(install);

    tokio::time::timeout(Duration::from_secs(1), async {
        while Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("dependency process survived handle drop");
}

#[tokio::test]
async fn explicit_windows_version_verb_has_no_restore_plan() {
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

    let prepared = prepare_install_packages(
        &binary,
        tmp.path().to_str().unwrap(),
        &["win7".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .unwrap();
    assert!(prepared.restore_plan().is_none());
    let outcome = prepared.spawn().unwrap().wait_and_restore().await;
    assert!(outcome.install_succeeded);
    assert_eq!(
        outcome.restore_state,
        PrefixVersionRestoreState::NotRequired
    );

    assert_eq!(fs::read_to_string(captured_calls).unwrap(), "-q win7\n");
}

#[tokio::test]
async fn install_rejects_uninitialized_prefix() {
    let tmp = tempdir().unwrap();
    let binary = make_fake_binary(tmp.path(), "winetricks", "#!/bin/sh\n");
    // Do NOT create pfx/ directory -- use a nonexistent path entirely.

    let result = install_packages(
        &binary,
        "/nonexistent/path/that/does/not/exist",
        &["vcrun2019".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, PrefixDepsError::PrefixNotInitialized { .. }),
        "expected PrefixNotInitialized, got: {err:?}"
    );
}

/// Process group id of `pid`, read from field 5 of `/proc/<pid>/stat`.
#[cfg(target_os = "linux")]
fn process_group_of(pid: u32) -> u32 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    // `comm` (field 2) may contain spaces; fields after the closing paren are
    // `state ppid pgrp ...`.
    let after_comm = &stat[stat.rfind(')').unwrap() + 1..];
    after_comm
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn into_parts_command_leads_its_process_group() {
    let tmp = tempdir().unwrap();
    let captured_args = tmp.path().join("args.txt");
    let binary = make_fake_binary(
        tmp.path(),
        "dep-tool",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\nexit 0\n",
            captured_args.display()
        ),
    );
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();

    let prepared = prepare_install_packages(
        &binary,
        tmp.path().to_str().unwrap(),
        &["win10".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .unwrap();
    let (mut command, restore_plan) = prepared.into_parts();
    assert!(restore_plan.is_none(), "explicit version verb has no plan");

    let mut child = command.spawn().unwrap();
    let pid = child.id().expect("child pid");
    // Not yet reaped, so `/proc/<pid>` exists even if the script has exited.
    assert_eq!(process_group_of(pid), pid, "install must lead its group");
    assert_ne!(
        process_group_of(std::process::id()),
        pid,
        "test process must not share the install group"
    );
    assert!(child.stdin.is_none(), "stdin must be null, not piped");
    assert!(child.stdout.is_some(), "stdout must be piped");
    assert!(child.stderr.is_some(), "stderr must be piped");

    let status = child.wait().await.unwrap();
    assert!(status.success());
    assert_eq!(fs::read_to_string(captured_args).unwrap(), "-q win10\n");
}
