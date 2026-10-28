use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::Duration;

use crosshook_core::community::{GitInvocation, GitRunError};
use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use super::super::runner::{
    default_runner_factory, run_test_program, run_test_program_with_drain_grace,
};

const DEADLINE: Duration = Duration::from_secs(10);
const OUTPUT_LIMIT: usize = 64 * 1024;

fn invocation(args: &[&str]) -> GitInvocation {
    GitInvocation {
        action: "test community runner",
        args: args.iter().map(OsString::from).collect(),
        env: BTreeMap::new(),
        directory: None,
    }
}

#[tokio::test]
async fn cancelled_runner_does_not_spawn() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let runner = default_runner_factory()(cancel);
    assert!(runner.is_cancelled());
    assert!(matches!(
        runner.run(invocation(&["--version"])).await,
        Err(GitRunError::Cancelled)
    ));
}

#[tokio::test]
async fn non_utf8_directory_is_rejected_before_spawning() {
    let runner = default_runner_factory()(CancellationToken::new());
    let mut invocation = invocation(&["--version"]);
    invocation.directory = Some(PathBuf::from(OsString::from_vec(vec![b'/', 0xff])));
    let error = runner.run(invocation).await.unwrap_err();
    assert!(matches!(
        error,
        GitRunError::Failed(error) if error.kind() == io::ErrorKind::InvalidInput
    ));
}

#[tokio::test]
async fn cancelling_sleep_child_kills_and_reaps_it() {
    let cancel = CancellationToken::new();
    let _cancel_on_failure = cancel.clone().drop_guard();
    let (started, wait_started) = oneshot::channel();
    let child_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        run_test_program(&child_cancel, "sleep", invocation(&["30"]), Some(started)).await
    });
    // Spawn notification is sent only after the real process has an assigned
    // pid. Cancellation does not race a guessed delay or filesystem poll.
    let pid = timeout(DEADLINE, wait_started)
        .await
        .expect("child spawn deadline")
        .expect("child spawned");
    let pid = Pid::from_raw(i32::try_from(pid).expect("child pid fits i32"));
    assert!(
        kill(pid, None).is_ok(),
        "child must exist before cancellation"
    );

    cancel.cancel();
    let result = timeout(DEADLINE, task)
        .await
        .expect("runner cancellation deadline")
        .expect("runner task completed");

    assert!(matches!(result, Err(GitRunError::Cancelled)), "{result:?}");
    assert_eq!(
        kill(pid, None),
        Err(Errno::ESRCH),
        "child survived cancellation"
    );
    assert_eq!(
        waitpid(pid, Some(WaitPidFlag::WNOHANG)),
        Err(Errno::ECHILD),
        "supervision must reap the direct child before returning"
    );
}

#[tokio::test]
async fn both_streams_are_drained_past_the_retention_limit() {
    let cancel = CancellationToken::new();
    let _cancel_on_failure = cancel.clone().drop_guard();
    // Each stream exceeds both the retention cap and a pipe's capacity. The
    // loop can finish only if both pipes keep being drained after the cap.
    let script = "i=0; \
        while [ \"$i\" -lt 4096 ]; do \
            printf '%s' 'oooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo'; \
            printf '%s' 'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee' >&2; \
            i=$((i + 1)); \
        done";
    let output = timeout(
        DEADLINE,
        run_test_program(&cancel, "sh", invocation(&["-c", script]), None),
    )
    .await
    .expect("both stream drains reached EOF")
    .expect("child completed");

    assert!(output.success);
    assert_eq!(output.stdout, "o".repeat(OUTPUT_LIMIT));
    assert_eq!(output.stderr, "e".repeat(OUTPUT_LIMIT));
}

#[tokio::test]
async fn gateway_forwards_environment_and_directory_and_keeps_nonzero_output() {
    let directory = tempfile::tempdir().unwrap();
    // The marker is addressed relatively so dropping the working directory
    // would change the observable result instead of silently passing.
    std::fs::write(directory.path().join("marker"), "fixture").unwrap();
    let mut invocation = invocation(&[
        "-c",
        "test -f marker || exit 99; printf '%s' \"$COMMUNITY_RUNNER_TEST\"; printf err >&2; exit 7",
    ]);
    invocation.directory = Some(directory.path().to_path_buf());
    invocation
        .env
        .insert("COMMUNITY_RUNNER_TEST".into(), "forwarded".into());
    let output = timeout(
        DEADLINE,
        run_test_program(&CancellationToken::new(), "sh", invocation, None),
    )
    .await
    .expect("child completion deadline")
    .expect("nonzero exit is captured output");

    assert!(!output.success);
    assert_eq!(output.stdout, "forwarded");
    assert_eq!(output.stderr, "err");
}

#[tokio::test]
async fn spawn_failure_is_a_typed_io_error() {
    let directory = tempfile::tempdir().unwrap();
    let missing_program = directory.path().join("missing-program");
    let result = run_test_program(
        &CancellationToken::new(),
        missing_program.to_str().unwrap(),
        invocation(&[]),
        None,
    )
    .await;
    assert!(matches!(
        result,
        Err(GitRunError::Failed(error)) if error.kind() == io::ErrorKind::NotFound
    ));
}

/// Script whose leader exits at once while a background `sleep` keeps the
/// inherited stdout/stderr pipes open. Its pid goes to the file named `$1`.
const BACKGROUND_HOLDER: &str = "printf out; sleep 30 & echo $! > \"$1\"; exit 0";

fn kill_background_holder(pid_file: &std::path::Path) {
    let pid: i32 = std::fs::read_to_string(pid_file)
        .expect("script recorded the background pid before exiting")
        .trim()
        .parse()
        .unwrap();
    let _ = kill(Pid::from_raw(pid), nix::sys::signal::Signal::SIGKILL);
}

#[tokio::test]
async fn drain_is_bounded_when_a_descendant_holds_the_pipes() {
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("pid");
    let cancel = CancellationToken::new();
    let _cancel_on_failure = cancel.clone().drop_guard();
    let invocation = invocation(&["-c", BACKGROUND_HOLDER, "sh", pid_file.to_str().unwrap()]);

    // The pipes never reach EOF (sleep holds them for 30 s); the production
    // drain grace must release the run. The deadline is only a watchdog.
    let result = timeout(DEADLINE, run_test_program(&cancel, "sh", invocation, None)).await;
    kill_background_holder(&pid_file);

    let output = result
        .expect("bounded drain must not wait for pipe EOF")
        .expect("leader exit is a normal completion");
    assert!(output.success);
    assert_eq!(output.stdout, "out");
}

#[tokio::test]
async fn cancel_after_leader_exit_resolves_while_a_descendant_holds_the_pipes() {
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("pid");
    let cancel = CancellationToken::new();
    let _cancel_on_failure = cancel.clone().drop_guard();
    let (started, wait_started) = oneshot::channel();
    let invocation = invocation(&["-c", BACKGROUND_HOLDER, "sh", pid_file.to_str().unwrap()]);
    let task_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        // A grace far beyond the watchdog: only cancellation can end the drain.
        run_test_program_with_drain_grace(
            &task_cancel,
            "sh",
            invocation,
            Some(started),
            Duration::from_secs(600),
        )
        .await
    });
    let pid = timeout(DEADLINE, wait_started)
        .await
        .expect("child spawn deadline")
        .expect("child spawned");
    let pid = Pid::from_raw(i32::try_from(pid).expect("child pid fits i32"));
    // The supervisor reaps the leader as soon as it exits (ESRCH afterwards);
    // yielding, not sleeping, lets that happen without a timing assumption.
    timeout(DEADLINE, async {
        while kill(pid, None).is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("leader exited");

    cancel.cancel();
    let result = timeout(DEADLINE, task)
        .await
        .expect("cancel after leader exit must resolve promptly")
        .expect("runner task completed");
    kill_background_holder(&pid_file);

    assert!(matches!(result, Err(GitRunError::Cancelled)), "{result:?}");
}
