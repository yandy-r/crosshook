//! S-2: restore and check children lead their own process group; a timeout
//! kills the whole group, and a descendant holding the output pipes after the
//! tool exited cannot hang the run.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use tempfile::tempdir;

use super::{make_fake_binary, write_registry_fixture};
use crate::prefix_deps::runner::bounded::Limits;
use crate::prefix_deps::runner::check::{build_check_command_with, run_check};
use crate::prefix_deps::runner::restore::restore_prefix_windows_version_with_limits;
use crate::prefix_deps::{
    PrefixDepsError, PrefixDepsTool, PrefixVersionRestorePlan, PrefixVersionRestoreState,
};

/// Upper bound so a regression fails instead of hanging the suite.
const HANG_GUARD: Duration = Duration::from_secs(20);
const SHORT_DRAIN: Duration = Duration::from_millis(100);

const WIN10: &str = "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"";

/// A script that backgrounds `sleep 300` with `stream` (1 or 2) still
/// attached, records its pid in `pid_file`, prints `stdout`, then exits 0 or
/// (with `hang`) waits for the grandchild.
fn pipe_holding_tool(dir: &Path, pid_file: &Path, stream: u8, stdout: &str, hang: bool) -> String {
    let other = if stream == 1 { 2 } else { 1 };
    let tail = if hang { "wait" } else { "exit 0" };
    make_fake_binary(
        dir,
        "pipe-holder",
        &format!(
            "#!/bin/sh\nsleep 300 {other}>/dev/null &\necho $! > '{}'\necho '{stdout}'\n{tail}\n",
            pid_file.display()
        ),
    )
}

fn read_pid(pid_file: &Path) -> Pid {
    let text = fs::read_to_string(pid_file).expect("grandchild pid recorded");
    Pid::from_raw(text.trim().parse().expect("pid"))
}

/// `/proc/<pid>/stat` state, `None` once the pid is gone.
fn proc_state(pid: Pid) -> Option<char> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

/// Whether `pid` is gone or a zombie within a bounded wait.
fn becomes_dead(pid: Pid) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if matches!(proc_state(pid), None | Some('Z')) {
            return true;
        }
        std::thread::yield_now();
    }
    false
}

fn plan(binary: String, prefix: &Path) -> PrefixVersionRestorePlan {
    PrefixVersionRestorePlan {
        binary_path: binary,
        prefix_path: prefix.to_string_lossy().into_owned(),
        tool_type: PrefixDepsTool::Winetricks,
        steam_app_id: None,
        restore_verb: "win10".to_string(),
    }
}

#[tokio::test]
async fn restore_returns_when_a_descendant_holds_stderr_after_exit() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(&pfx, WIN10);
    let pid_file = tmp.path().join("grandchild.pid");
    let binary = pipe_holding_tool(tmp.path(), &pid_file, 2, "", false);

    let outcome = tokio::time::timeout(
        HANG_GUARD,
        restore_prefix_windows_version_with_limits(
            &plan(binary, tmp.path()),
            Limits {
                timeout: HANG_GUARD,
                drain_grace: SHORT_DRAIN,
            },
        ),
    )
    .await
    .expect("the restore must not wait for the pipe holder");

    let grandchild = read_pid(&pid_file);
    let _ = kill(grandchild, Signal::SIGKILL);
    assert_eq!(outcome.state, PrefixVersionRestoreState::Succeeded);
}

#[tokio::test]
async fn restore_timeout_kills_the_whole_process_group() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(&pfx, WIN10);
    let pid_file = tmp.path().join("grandchild.pid");
    let binary = pipe_holding_tool(tmp.path(), &pid_file, 2, "", true);

    let outcome = restore_prefix_windows_version_with_limits(
        &plan(binary, tmp.path()),
        Limits {
            timeout: Duration::from_secs(1),
            drain_grace: SHORT_DRAIN,
        },
    )
    .await;

    assert_eq!(outcome.state, PrefixVersionRestoreState::Failed);
    assert!(outcome
        .error
        .as_deref()
        .is_some_and(|error| error.contains("timed out")));
    let grandchild = read_pid(&pid_file);
    let dead = becomes_dead(grandchild);
    let _ = kill(grandchild, Signal::SIGKILL);
    assert!(dead, "the restore's grandchild survived the timeout");
}

#[tokio::test]
async fn check_returns_when_a_descendant_holds_stdout_after_exit() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();
    let pid_file = tmp.path().join("grandchild.pid");
    let binary = pipe_holding_tool(tmp.path(), &pid_file, 1, "vcrun2019 dotnet48", false);
    let command = build_check_command_with(
        &binary,
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Winetricks,
        None,
        false,
    )
    .unwrap();

    let result = tokio::time::timeout(
        HANG_GUARD,
        run_check(
            command,
            Limits {
                timeout: HANG_GUARD,
                drain_grace: SHORT_DRAIN,
            },
        ),
    )
    .await
    .expect("the check must not wait for the pipe holder");

    let grandchild = read_pid(&pid_file);
    let _ = kill(grandchild, Signal::SIGKILL);
    assert_eq!(result.unwrap(), vec!["vcrun2019", "dotnet48"]);
}

#[tokio::test]
async fn check_timeout_kills_the_whole_process_group() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();
    let pid_file = tmp.path().join("grandchild.pid");
    let binary = pipe_holding_tool(tmp.path(), &pid_file, 1, "", true);
    let command = build_check_command_with(
        &binary,
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Winetricks,
        None,
        false,
    )
    .unwrap();

    let result = run_check(
        command,
        Limits {
            timeout: Duration::from_secs(1),
            drain_grace: SHORT_DRAIN,
        },
    )
    .await;

    assert!(
        matches!(result, Err(PrefixDepsError::Timeout { .. })),
        "{result:?}"
    );
    let grandchild = read_pid(&pid_file);
    let dead = becomes_dead(grandchild);
    let _ = kill(grandchild, Signal::SIGKILL);
    assert!(dead, "the check's grandchild survived the timeout");
}
