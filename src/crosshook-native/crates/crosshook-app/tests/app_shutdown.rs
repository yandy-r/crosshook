//! Integration tests for `App::shutdown`: the explicit, bounded shutdown a
//! shell calls before its event loop exits the process (Tauri ends with
//! `process::exit`, so `App`'s drop never runs).
//!
//! Supervised children are fake `#!/bin/sh` scripts in a tempdir; readiness
//! is a oneshot handshake, never a sleep.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use crosshook_app::jobs::{supervise_child, JobEventKind, JobSpec, Outcome, DEFAULT_KILL_GRACE};
use crosshook_app::{App, AppError, AppErrorKind};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::runtime::Handle;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

/// Upper bound for awaits that must not hang.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Pids of live (non-zombie) processes in process group `pgid`, from
/// `/proc/<pid>/stat` (`pid (comm) state ppid pgrp ...`).
fn live_group_members(pgid: u32) -> Vec<u32> {
    let pgid = pgid.to_string();
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let mut fields = stat.rsplit_once(')')?.1.split_ascii_whitespace();
            let state = fields.next()?;
            let pgrp = fields.nth(1)?;
            (pgrp == pgid && !matches!(state, "Z" | "X")).then_some(pid)
        })
        .collect()
}

/// Writes an executable fake installer: it starts a long-lived grandchild in
/// its own process group, prints `ready`, then waits.
fn write_fake_installer(dir: &Path) -> PathBuf {
    let path = dir.join("fake-installer");
    std::fs::write(&path, "#!/bin/sh\nsleep 30 &\necho ready\nwait\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Spawns `script` as a process-group leader, reports its pid once it printed
/// `ready`, and supervises it on `cancel` like the process services do.
async fn run_supervised(
    script: PathBuf,
    cancel: CancellationToken,
    ready: oneshot::Sender<u32>,
) -> io::Result<Outcome> {
    let mut child = Command::new(&script)
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut line = String::new();
    stdout.read_line(&mut line).await?;
    assert_eq!(line.trim(), "ready");
    let _ = ready.send(child.id().expect("running child has a pid"));
    let exit = supervise_child(child, &cancel, DEFAULT_KILL_GRACE).await?;
    Ok(if exit.was_cancelled() {
        Outcome::Cancelled
    } else {
        Outcome::Succeeded
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_terminates_supervised_process_group() {
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_installer(dir.path());
    let app = App::with_handle(Handle::current());
    let (ready_tx, ready_rx) = oneshot::channel();
    let mut handle = app
        .jobs()
        .start(JobSpec::new("fake-install"), move |ctx| async move {
            match run_supervised(script, ctx.cancel_token().clone(), ready_tx).await {
                Ok(outcome) => outcome,
                Err(error) => Outcome::Failed(AppError::from(error)),
            }
        })
        .unwrap();
    let pgid = tokio::time::timeout(TIMEOUT, ready_rx)
        .await
        .expect("child ready in time")
        .expect("job reports the child pid");
    assert_eq!(
        live_group_members(pgid).len(),
        2,
        "fake installer and its grandchild must be running"
    );

    let started = Instant::now();
    let finished = tokio::time::timeout(TIMEOUT, app.shutdown(App::DEFAULT_SHUTDOWN_TIMEOUT))
        .await
        .expect("shutdown must not hang");
    assert!(finished, "every job must finish within the shutdown bound");
    assert!(started.elapsed() < App::DEFAULT_SHUTDOWN_TIMEOUT);
    // The leader and its grandchild are gone, not just signalled.
    assert_eq!(live_group_members(pgid), Vec::<u32>::new());

    let event = tokio::time::timeout(TIMEOUT, async {
        loop {
            match handle.recv().await {
                Some(event) if matches!(event.kind, JobEventKind::Finished(_)) => break event,
                Some(_) => continue,
                None => panic!("stream ended before Finished"),
            }
        }
    })
    .await
    .expect("Finished in time");
    assert_eq!(event.kind, JobEventKind::Finished(Outcome::Cancelled));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_is_bounded_when_a_job_ignores_cancel() {
    let app = App::with_handle(Handle::current());
    let (started_tx, started_rx) = oneshot::channel();
    let handle = app
        .jobs()
        .start(JobSpec::new("stuck"), |_ctx| async move {
            let _ = started_tx.send(());
            std::future::pending::<Outcome>().await
        })
        .unwrap();
    started_rx.await.unwrap();

    let bound = Duration::from_millis(300);
    let started = Instant::now();
    let finished = tokio::time::timeout(TIMEOUT, app.shutdown(bound))
        .await
        .expect("shutdown must not exceed its bound");
    assert!(
        !finished,
        "a job ignoring cancel must report a timed-out wait"
    );
    assert!(started.elapsed() >= bound);
    assert!(handle.cancel_token().is_cancelled());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_cancels_app_tasks_and_returns_when_idle() {
    let app = App::with_handle(Handle::current());
    let task = app.spawn(std::future::pending::<()>());
    let finished = tokio::time::timeout(TIMEOUT, app.shutdown(App::DEFAULT_SHUTDOWN_TIMEOUT))
        .await
        .expect("idle shutdown returns at once");
    assert!(finished);
    let error = tokio::time::timeout(TIMEOUT, task)
        .await
        .expect("task resolves")
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Cancelled);
}
