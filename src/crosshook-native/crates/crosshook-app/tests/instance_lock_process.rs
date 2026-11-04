//! Real-process single-instance lock checks.
//!
//! The parent tests live in `mod instance_lock`, so the spec's
//! `cargo test -p crosshook-app instance_lock` runs them. Child processes
//! re-execute this test binary with `--exact <child_test>` and an environment
//! variable naming the tempdir runtime dir; each child test returns at once
//! when its variable is unset, so a normal run treats them as no-ops.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crosshook_app::instance_lock::{
    AcquireError, InstanceLock, InstanceLockPaths, HANDOFF_ENV, HANDOFF_WAIT,
};

const CHILD_DIR_ENV: &str = "CROSSHOOK_LOCK_TEST_DIR";
const CHILD_HOLD: &str = "child_hold_lock";
const CHILD_HANDOFF: &str = "child_wait_handoff";

fn child_dir() -> Option<PathBuf> {
    std::env::var_os(CHILD_DIR_ENV).map(PathBuf::from)
}

fn child_command(test: &str, dir: &Path) -> Command {
    let mut cmd = Command::new(std::env::current_exe().expect("test binary path"));
    cmd.args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .env(CHILD_DIR_ENV, dir)
        .env_remove(HANDOFF_ENV);
    cmd
}

/// Waits for the child's `marker` on stdout. libtest prints
/// `test <name> ... ` first, so the marker ends the line.
fn wait_for_marker(child: &mut std::process::Child, marker: &str) {
    let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let seen = stdout
        .lines()
        .map_while(Result::ok)
        .any(|line| line.ends_with(marker));
    if !seen {
        let _ = child.kill();
    }
    assert!(seen, "child never printed {marker}");
}

/// Takes a lock that should be free. A process forked at that moment (a
/// failed spawn's child, or another test's) briefly shares the lock's open
/// file description until it execs or exits; the wait wakes on that last
/// close (`IN_CLOSE`), so this stays exact without sleeping.
fn acquire_soon(paths: &InstanceLockPaths, what: &str) -> InstanceLock {
    InstanceLock::acquire_with_wait(paths, std::time::Duration::from_secs(2))
        .unwrap_or_else(|error| panic!("{what}: {error:?}"))
}

fn assert_contended(result: Result<InstanceLock, AcquireError>) {
    assert!(
        matches!(result, Err(AcquireError::Contended)),
        "expected Contended, got {result:?}"
    );
}

/// Child: holds the lock and blocks on stdin until killed.
#[test]
fn child_hold_lock() {
    let Some(dir) = child_dir() else { return };
    let _lock = InstanceLock::try_acquire(&InstanceLockPaths::in_dir(dir)).expect("child lock");
    println!("LOCKED");
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
}

/// Child: hands off to a waiting process while keeping its lock, then
/// "shuts down" (blocks on stdin) until killed, like the Classic exit path.
#[test]
fn child_wait_handoff() {
    let Some(dir) = child_dir() else { return };
    if std::env::var(HANDOFF_ENV).as_deref() == Ok("1") {
        InstanceLock::acquire_with_wait(&InstanceLockPaths::in_dir(dir), HANDOFF_WAIT)
            .expect("handed-off child must get the lock");
        println!("TOOK_OVER");
        return;
    }
    let paths = InstanceLockPaths::in_dir(&dir);
    let lock = InstanceLock::try_acquire(&paths).expect("first process lock");
    let next = lock
        .spawn_handoff(child_command(CHILD_HANDOFF, &dir))
        .expect("spawn handed-off child");
    println!("HANDED_OFF {}", next.id());
    // The new process outlives this one by design (it waits for our exit).
    std::mem::forget(next);
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
}

mod instance_lock {
    use super::*;

    /// Acceptance: after `kill -9` of the holder, the next start acquires
    /// the lock immediately (no stale-lock cleanup).
    #[test]
    fn kill_9_releases_lock() {
        let runtime = tempfile::tempdir().unwrap();
        let paths = InstanceLockPaths::in_dir(runtime.path());
        let mut child = child_command(CHILD_HOLD, runtime.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        wait_for_marker(&mut child, "LOCKED");
        assert_contended(InstanceLock::try_acquire(&paths));
        child.kill().unwrap();
        child.wait().unwrap();
        InstanceLock::try_acquire(&paths).expect("lock free right after SIGKILL");
    }

    /// Whether process `pid` has any descriptor open on `file`.
    fn has_fd_on(pid: u32, file: &Path) -> bool {
        let target = std::fs::canonicalize(file).unwrap();
        std::fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read_link(entry.path()).ok())
            .any(|link| link == target)
    }

    /// The lock descriptors are close-on-exec: a spawned child (a game) never
    /// holds them, so the lock dies with CrossHook.
    #[test]
    fn lock_fd_not_inherited() {
        let runtime = tempfile::tempdir().unwrap();
        let paths = InstanceLockPaths::in_dir(runtime.path());
        let lock = InstanceLock::try_acquire(&paths).unwrap();
        let mut cat = Command::new("cat").stdin(Stdio::piped()).spawn().unwrap();
        let shared = paths.shared_file().unwrap();
        assert!(has_fd_on(std::process::id(), paths.file()));
        assert!(has_fd_on(std::process::id(), shared));
        assert!(
            !has_fd_on(cat.id(), paths.file()) && !has_fd_on(cat.id(), shared),
            "spawned child inherited a lock descriptor"
        );
        drop(lock);
        // `cat` stays alive past this bound, so a leaked descriptor fails here.
        acquire_soon(&paths, "lock free while the child still runs");
        cat.kill().unwrap();
        cat.wait().unwrap();
    }

    /// The handing-off process keeps the lock while it shuts down; the new
    /// process waits and takes it as soon as the old one exits. The new
    /// process inherits the old one's stdout, so its marker arrives on the
    /// same pipe.
    #[test]
    fn handoff_waits_for_the_old_process_to_exit() {
        let runtime = tempfile::tempdir().unwrap();
        let paths = InstanceLockPaths::in_dir(runtime.path());
        let mut first = child_command(CHILD_HANDOFF, runtime.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(first.stdout.take().unwrap())
            .lines()
            .map_while(Result::ok);
        assert!(
            lines.any(|line| line.contains("HANDED_OFF")),
            "first process did not hand off"
        );
        // Still shutting down: the lock is not released early.
        assert_contended(InstanceLock::try_acquire(&paths));
        first.kill().unwrap();
        first.wait().unwrap();
        assert!(
            lines.any(|line| line.ends_with("TOOK_OVER")),
            "handed-off process did not take the lock"
        );
    }

    /// A failed handoff spawn changes nothing: the lock is still held.
    #[test]
    fn failed_handoff_spawn_keeps_the_lock() {
        let runtime = tempfile::tempdir().unwrap();
        let paths = InstanceLockPaths::in_dir(runtime.path());
        let lock = InstanceLock::try_acquire(&paths).unwrap();
        let error = lock
            .spawn_handoff(Command::new(runtime.path().join("no-such-binary")))
            .expect_err("spawning a missing binary fails");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert_contended(InstanceLock::try_acquire(&paths));
        drop(lock);
        acquire_soon(&paths, "released once the lock drops");
    }
}
