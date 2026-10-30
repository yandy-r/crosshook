//! Tests for child supervision and the moved process helpers.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use std::{fs, io};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio_util::sync::CancellationToken;

use super::sweep::{
    environ_references_prefix, kill_processes_using_prefix_with, protected_pids, PrefixTarget,
};
use super::{
    processes_using_prefix, slugify_target, supervise_child, supervise_child_with_kill, ChildExit,
};

const GRACE: Duration = Duration::from_millis(200);
/// A grace no kill-path test may wait out.
const LONG_GRACE: Duration = Duration::from_secs(60);
/// Upper bound for every await that must not hang.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Spawns `command` (which must print `ready` first) in its own process group
/// and returns it with its stdout reader positioned after the `ready` line.
async fn spawn_ready_command(mut command: Command) -> (Child, BufReader<ChildStdout>) {
    let mut child = command
        .stdout(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .expect("spawn ready command");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut line = String::new();
    stdout.read_line(&mut line).await.expect("read ready line");
    assert_eq!(line.trim(), "ready");
    (child, stdout)
}

/// Spawns `sh -c script` in its own process group and waits until the script
/// prints its first line, so signal dispositions are installed before cancel.
async fn spawn_ready(script: &str) -> Child {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    spawn_ready_command(command).await.0
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_sigkills_group_without_waiting_for_grace() {
    // Ignores SIGTERM: only SIGKILL can end it.
    let child = spawn_ready("trap '' TERM; echo ready; sleep 30").await;
    let cancel = CancellationToken::new();
    let kill = CancellationToken::new();
    kill.cancel();
    let exit = tokio::time::timeout(
        TIMEOUT,
        supervise_child_with_kill(child, &cancel, &kill, LONG_GRACE),
    )
    .await
    .expect("kill must not wait out the grace")
    .unwrap();
    assert!(matches!(exit, ChildExit::Killed(_)), "{exit:?}");
    assert_eq!(exit.status().signal(), Some(libc_sigkill()));
    assert!(exit.was_cancelled());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_during_term_grace_sigkills_immediately() {
    // The handler survives SIGTERM and reports it, so the test knows the
    // supervisor is inside the TERM grace before firing `kill`. The builtin
    // `read` (stdin held open by the child handle) lets the shell run the
    // trap at once; a foreground external command would defer it.
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg("trap 'echo term' TERM; echo ready; while :; do read line; done")
        .stdin(Stdio::piped());
    let (child, mut stdout) = spawn_ready_command(command).await;
    let cancel = CancellationToken::new();
    let kill = CancellationToken::new();
    let supervision = {
        let (cancel, kill) = (cancel.clone(), kill.clone());
        tokio::spawn(
            async move { supervise_child_with_kill(child, &cancel, &kill, LONG_GRACE).await },
        )
    };
    cancel.cancel();
    let mut line = String::new();
    tokio::time::timeout(TIMEOUT, stdout.read_line(&mut line))
        .await
        .expect("SIGTERM reaches the group")
        .expect("read term line");
    assert_eq!(line.trim(), "term");
    assert!(!supervision.is_finished(), "grace must still be running");

    kill.cancel();
    let exit = tokio::time::timeout(TIMEOUT, supervision)
        .await
        .expect("kill must cut the grace short")
        .expect("supervision task")
        .unwrap();
    assert!(matches!(exit, ChildExit::Killed(_)), "{exit:?}");
    assert_eq!(exit.status().signal(), Some(libc_sigkill()));
}

/// Spawns `env <var>=<value> sh` blocked on `read`, so its environ is stable.
async fn spawn_with_env(var: &str, value: &Path) -> Child {
    let mut command = Command::new("env");
    command
        .arg(format!("{var}={}", value.display()))
        .arg("sh")
        .arg("-c")
        .arg("echo ready; read line")
        .stdin(Stdio::piped());
    spawn_ready_command(command).await.0
}

/// A canonical `<dir>/real/compat/pfx` prefix and a `<dir>/alias` symlink to
/// `<dir>/real`.
fn prefix_with_alias(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let real = dir.join("real");
    fs::create_dir_all(real.join("compat/pfx")).unwrap();
    std::os::unix::fs::symlink(&real, dir.join("alias")).unwrap();
    let prefix = fs::canonicalize(real.join("compat/pfx")).unwrap();
    (prefix, dir.join("alias"))
}

fn references(environ: &[u8], prefix: &Path) -> bool {
    let target = PrefixTarget::new(prefix).expect("non-empty prefix");
    environ_references_prefix(environ, &target, &mut Default::default())
}

#[test]
fn environ_matches_only_prefix_variables_by_canonical_path() {
    let dir = tempfile::tempdir().unwrap();
    let (prefix, alias) = prefix_with_alias(dir.path());
    let entry = |key: &str, value: &Path| format!("{key}={}\0", value.display()).into_bytes();

    assert!(references(&entry("WINEPREFIX", &prefix), &prefix));
    // A symlinked alias of the same prefix.
    assert!(references(
        &entry("WINEPREFIX", &alias.join("compat/pfx")),
        &prefix
    ));
    // The Proton compat-data root whose `pfx` is the prefix.
    assert!(references(
        &entry("STEAM_COMPAT_DATA_PATH", &alias.join("compat")),
        &prefix
    ));
    // The `pfx` of a compat-data root target.
    assert!(references(
        &entry("WINEPREFIX", &prefix),
        prefix.parent().expect("compat root")
    ));
    // Other variables, siblings, children and relative values never match.
    assert!(!references(&entry("HOME", &prefix), &prefix));
    assert!(!references(&entry("WINEDLLPATH", &prefix), &prefix));
    assert!(!references(
        &entry("WINEPREFIX", &prefix.with_file_name("pfx-other")),
        &prefix
    ));
    assert!(!references(
        &entry("WINEPREFIX", &prefix.join("drive_c")),
        &prefix
    ));
    assert!(!references(b"WINEPREFIX=compat/pfx\0", &prefix));
    // A prefix-sized home folder does not match every process's `HOME`.
    assert!(!references(&entry("HOME", dir.path()), dir.path()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn processes_using_prefix_matches_aliases_but_not_other_variables() {
    let dir = tempfile::tempdir().unwrap();
    let (prefix, alias) = prefix_with_alias(dir.path());
    let user = spawn_with_env("WINEPREFIX", &alias.join("compat/pfx")).await;
    let home = spawn_with_env("HOME", &prefix).await;
    let sibling = spawn_with_env("WINEPREFIX", &prefix.with_file_name("pfx-other")).await;
    let user_pid = user.id().expect("user pid");

    let pids = processes_using_prefix(&prefix);

    assert!(pids.contains(&user_pid), "{user_pid} not in {pids:?}");
    assert!(!pids.contains(&home.id().expect("home pid")), "{pids:?}");
    assert!(
        !pids.contains(&sibling.id().expect("sibling pid")),
        "{pids:?}"
    );
    assert!(processes_using_prefix(Path::new("")).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_sweep_signals_locally_and_spares_protected_pids() {
    let dir = tempfile::tempdir().unwrap();
    let (prefix, _) = prefix_with_alias(dir.path());
    let mut user = spawn_with_env("WINEPREFIX", &prefix).await;
    let mut protected_user = spawn_with_env("WINEPREFIX", &prefix).await;
    let protected = std::collections::HashSet::from([protected_user.id().expect("pid")]);

    kill_processes_using_prefix_with(&prefix, false, &protected);

    let status = tokio::time::timeout(TIMEOUT, user.wait())
        .await
        .expect("swept process exits")
        .unwrap();
    assert_eq!(status.signal(), Some(libc_sigkill()));
    assert!(
        protected_user.try_wait().unwrap().is_none(),
        "a protected pid was signalled"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_sweep_is_skipped_under_flatpak() {
    let dir = tempfile::tempdir().unwrap();
    let (prefix, _) = prefix_with_alias(dir.path());
    let mut user = spawn_with_env("WINEPREFIX", &prefix).await;

    kill_processes_using_prefix_with(&prefix, true, &Default::default());

    assert!(processes_using_prefix(&prefix).contains(&user.id().expect("pid")));
    assert!(user.try_wait().unwrap().is_none(), "swept under Flatpak");
}

#[test]
fn protected_pids_cover_the_process_and_its_ancestors() {
    let protected = protected_pids();
    assert!(protected.contains(&std::process::id()));
    let parent = std::os::unix::process::parent_id();
    assert!(
        protected.contains(&parent),
        "parent {parent} in {protected:?}"
    );
}
