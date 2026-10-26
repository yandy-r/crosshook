//! Tests for child supervision and the moved process helpers.

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

use super::{
    create_log_path, create_log_path_in, environ_entry_contains_prefix_path, slugify_target,
    supervise_child, ChildExit, LOG_DIR,
};

const GRACE: Duration = Duration::from_millis(200);

/// Spawns `sh -c script` in its own process group and waits until the script
/// prints its first line, so signal dispositions are installed before cancel.
async fn spawn_ready(script: &str) -> Child {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sh");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .await
        .expect("read ready line");
    assert_eq!(line.trim(), "ready");
    child
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn natural_exit_is_reported_without_signals() {
    let child = spawn_ready("echo ready; exit 3").await;
    let cancel = CancellationToken::new();
    let exit = supervise_child(child, &cancel, GRACE).await.unwrap();
    assert_eq!(exit, ChildExit::Natural(exit.status()));
    assert_eq!(exit.status().code(), Some(3));
    assert!(!exit.was_cancelled());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_terminates_group_gracefully() {
    let child = spawn_ready("echo ready; sleep 30").await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let started = Instant::now();
    let exit = supervise_child(child, &cancel, Duration::from_secs(10))
        .await
        .unwrap();
    assert!(matches!(exit, ChildExit::Terminated(_)), "{exit:?}");
    assert_eq!(exit.status().signal(), Some(libc_sigterm()));
    // Did not wait out the grace period.
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_force_kills_group_ignoring_sigterm() {
    let child = spawn_ready("trap '' TERM; echo ready; sleep 30").await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let started = Instant::now();
    let exit = supervise_child(child, &cancel, GRACE).await.unwrap();
    assert!(matches!(exit, ChildExit::Killed(_)), "{exit:?}");
    assert_eq!(exit.status().signal(), Some(libc_sigkill()));
    assert!(started.elapsed() >= GRACE);
}

/// `/proc/<pid>/stat` state, or `None` once the pid is gone.
fn proc_state(pid: u32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // `pid (comm) S ...`; comm may contain spaces/parens.
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

/// Waits until `pid` is gone or a zombie (dead, awaiting its new parent).
async fn wait_dead(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if matches!(proc_state(pid), None | Some('Z')) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

async fn read_pid_file(path: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            return text.trim().parse().expect("pid file holds a pid");
        }
        assert!(Instant::now() < deadline, "grandchild never wrote its pid");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_kills_group_member_outliving_its_leader() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("grandchild.pid");
    // The leader dies on SIGTERM; the grandchild ignores it (the disposition
    // survives `exec`) and publishes its pid atomically.
    let script = format!(
        "sh -c 'trap \"\" TERM; echo $$ > \"$0.tmp\" && mv \"$0.tmp\" \"$0\"; exec sleep 30' '{}' & \
         echo ready; wait",
        pid_file.display()
    );
    let child = spawn_ready(&script).await;
    let grandchild = read_pid_file(&pid_file).await;
    assert!(!matches!(proc_state(grandchild), None | Some('Z')));

    let cancel = CancellationToken::new();
    cancel.cancel();
    let exit = supervise_child(child, &cancel, GRACE).await.unwrap();
    assert!(matches!(exit, ChildExit::Killed(_)), "{exit:?}");
    // The leader itself went down on SIGTERM.
    assert_eq!(exit.status().signal(), Some(libc_sigterm()));
    assert!(wait_dead(grandchild).await, "grandchild survived cancel");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_group_leader_is_rejected_without_signals() {
    // Shares the test process group: signalling its group would SIGTERM the
    // test runner itself.
    let child = Command::new("sleep")
        .arg("30")
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sleep");
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = supervise_child(child, &cancel, GRACE).await.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exited_child_is_natural_even_if_already_cancelled() {
    let child = spawn_ready("echo ready; exit 3").await;
    let pid = child.id().unwrap();
    // Exited (zombie) before supervision sees the cancelled token.
    assert!(wait_dead(pid).await);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let exit = supervise_child(child, &cancel, GRACE).await.unwrap();
    assert_eq!(exit, ChildExit::Natural(exit.status()));
    assert_eq!(exit.status().code(), Some(3));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aborting_supervision_kills_the_group() {
    // No `kill_on_drop`: only the supervision guard may kill it.
    let child = Command::new("sleep")
        .arg("30")
        .process_group(0)
        .spawn()
        .expect("spawn sleep");
    let pid = child.id().unwrap();
    let (started, wait_started) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let cancel = CancellationToken::new();
        let mut started = Some(started);
        tokio::select! {
            // Polled first, so supervision is armed and pending before
            // `started` fires.
            biased;
            exit = supervise_child(child, &cancel, GRACE) => exit,
            () = std::future::poll_fn(|_| {
                if let Some(started) = started.take() {
                    let _ = started.send(());
                }
                std::task::Poll::Pending
            }) => unreachable!(),
        }
    });
    wait_started.await.unwrap();
    assert!(!matches!(proc_state(pid), None | Some('Z')));
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(wait_dead(pid).await, "child survived aborted supervision");
}

fn libc_sigterm() -> i32 {
    nix::sys::signal::Signal::SIGTERM as i32
}

fn libc_sigkill() -> i32 {
    nix::sys::signal::Signal::SIGKILL as i32
}

#[test]
fn slugify_maps_non_alphanumerics_and_falls_back() {
    assert_eq!(slugify_target("  My Game: GOTY! ", "x"), "my-game--goty");
    assert_eq!(slugify_target("!!!", "install"), "install");
}

#[test]
fn prefix_path_rejects_sibling_with_extra_suffix_after_match() {
    let prefix = b"/foo/bar";
    assert!(!environ_entry_contains_prefix_path(
        b"WINEPREFIX=/foo/bar-baz",
        prefix
    ));
}

#[test]
fn prefix_path_accepts_exact_match_and_subdirectory() {
    let prefix = b"/foo/bar";
    assert!(environ_entry_contains_prefix_path(
        b"WINEPREFIX=/foo/bar",
        prefix
    ));
    assert!(environ_entry_contains_prefix_path(b"/foo/bar", prefix));
    assert!(environ_entry_contains_prefix_path(
        b"WINEPREFIX=/foo/bar/pfx",
        prefix
    ));
}

#[test]
fn prefix_path_accepts_colon_separated_path_list() {
    let prefix = b"/foo/bar";
    assert!(environ_entry_contains_prefix_path(
        b"PATH=/a:/foo/bar:/c",
        prefix
    ));
}

#[test]
fn create_log_path_writes_into_the_shared_tmp_directory() {
    let path = create_log_path("logdir-prod", "slug").expect("production log path");

    assert_eq!(path.parent(), Some(Path::new(LOG_DIR)));
    assert!(path.is_file());
    let _ = std::fs::remove_file(path);
}

#[test]
fn create_log_path_in_creates_the_directory_and_a_unique_file() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("deep/logs");

    let path = create_log_path_in(nested.clone(), "game", "quest")
        .expect("log file in the given directory");

    assert_eq!(path.parent(), Some(nested.as_path()));
    assert!(path.is_file(), "the log file exists and is empty");
    assert!(path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("game-quest-"));
}

#[test]
fn create_log_path_in_never_truncates_an_existing_same_millisecond_file() {
    let dir = tempfile::tempdir().unwrap();
    // Two sessions in the same millisecond resolve to the same stem; the
    // second call must not open (and truncate) the first file.
    let first =
        create_log_path_in(dir.path().to_path_buf(), "game", "twin").expect("first log file");
    std::fs::write(&first, b"first-session-bytes").unwrap();

    let second =
        create_log_path_in(dir.path().to_path_buf(), "game", "twin").expect("second log file");

    assert_ne!(first, second, "the second session gets its own file");
    assert_eq!(
        std::fs::read(&first).unwrap(),
        b"first-session-bytes",
        "the first session's log is never truncated"
    );
    assert!(second.is_file());
}
