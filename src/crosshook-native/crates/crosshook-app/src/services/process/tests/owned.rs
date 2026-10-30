//! `start_owned`: spawn failure fails the start, a cancel before the bind
//! spawns nothing, and `after_exit` runs before the completion.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::process::Command;

use crate::jobs::{create_log_file_in, JobRegistry, JobSpec};
use crate::services::process::child::LogFile;
use crate::services::process::owned::{start_owned, FailureText, OwnedRun};
use crate::services::process::owner::ActiveJobs;
use crate::services::process::{JobOwner, ProcessCompletion, RunEvent};
use crate::AppErrorKind;

const TEXT: FailureText = FailureText {
    launch: |message| format!("launch failed: {message}"),
    exited: |status| format!("exited with {status:?}"),
};

fn owned_run(dir: &Path, command: Command) -> (OwnedRun, PathBuf) {
    let (path, file) = create_log_file_in(dir.to_path_buf(), "test", "owned").unwrap();
    let log_path = path.clone();
    (
        OwnedRun {
            command,
            log: LogFile { path, file },
            text: TEXT,
        },
        log_path,
    )
}

fn sh(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_failure_fails_the_start_and_removes_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = JobRegistry::default();
    let active = ActiveJobs::default();
    let slot = active.claim(JobOwner::new("main")).unwrap();
    let (run, log_path) = owned_run(dir.path(), Command::new(dir.path().join("missing")));
    let after = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&after);

    let error = start_owned(
        &jobs,
        JobSpec::new("test-owned"),
        slot,
        run,
        move |killed| {
            seen.lock().unwrap().push(killed);
            async {}
        },
    )
    .await
    .err()
    .expect("spawn failure fails the start");

    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(error.message().starts_with("launch failed: "));
    assert!(!log_path.exists(), "the unused log is removed");
    assert_eq!(*after.lock().unwrap(), vec![false]);
    assert_eq!(active.len(), 0, "the owner slot is released");
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_before_the_job_started_spawns_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let jobs = JobRegistry::default();
    let active = ActiveJobs::default();
    let owner = JobOwner::new("main");
    let slot = active.claim(owner.clone()).unwrap();
    // The run is still being prepared: the cancel is parked on the slot.
    assert!(active.cancel(&owner, &jobs));
    let (run, log_path) = owned_run(dir.path(), sh(&format!("touch '{}'", marker.display())));

    let error = start_owned(&jobs, JobSpec::new("test-owned"), slot, run, |_| async {})
        .await
        .err()
        .expect("cancelled before start");

    assert_eq!(error.kind(), AppErrorKind::Cancelled);
    assert!(!marker.exists(), "nothing may run after a cancel");
    assert!(!log_path.exists());
    assert_eq!(active.len(), 0);
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn after_exit_runs_with_the_kill_flag_before_the_completion() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = JobRegistry::default();
    let active = ActiveJobs::default();
    let owner = JobOwner::new("main");
    let slot = active.claim(owner.clone()).unwrap();
    let (run, _) = owned_run(dir.path(), sh("trap '' TERM; echo ready; exec sleep 30"));
    let after = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&after);

    let mut run = start_owned(
        &jobs,
        JobSpec::new("test-owned"),
        slot,
        run,
        move |killed| {
            seen.lock().unwrap().push(killed);
            async {}
        },
    )
    .await
    .unwrap();
    assert!(matches!(run.next().await, Some(RunEvent::Log(_))));
    assert!(active.kill(&owner));

    let completion = loop {
        match run.next().await.expect("Finished") {
            RunEvent::Log(_) => {}
            RunEvent::Finished(completion) => break completion,
        }
    };
    assert_eq!(
        completion,
        Some(ProcessCompletion {
            code: None,
            signal: Some(9)
        })
    );
    assert_eq!(*after.lock().unwrap(), vec![true]);
    assert_eq!(active.len(), 0);
    assert_eq!(jobs.job_counts(), (0, 0));
}
