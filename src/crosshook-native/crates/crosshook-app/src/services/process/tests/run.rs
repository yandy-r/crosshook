//! `start_run` / `StartingRun::started` / `ProcessRun::next` ordering.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{oneshot, Notify};

use crate::jobs::{JobKey, JobProgress, JobRegistry, JobSpec, Outcome};
use crate::log_tail::LogBatch;
use crate::services::process::run::start_run;
use crate::services::process::RunEvent;
use crate::{AppError, AppErrorKind};

fn lines(batch: &[&str]) -> LogBatch {
    LogBatch::Lines(batch.iter().map(|line| (*line).to_string()).collect())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn started_returns_before_the_first_log_line() {
    let jobs = JobRegistry::default();
    let gate = Arc::new(Notify::new());
    let body_gate = Arc::clone(&gate);
    let starting = start_run(
        &jobs,
        JobSpec::new("test-run"),
        move |ctx, completion: oneshot::Sender<u32>, started| async move {
            started.send(()).unwrap();
            // No output until the test, holding the run, opens the gate.
            body_gate.notified().await;
            ctx.log(vec!["first".to_string()]);
            completion.send(7).unwrap();
            Outcome::Succeeded
        },
    )
    .unwrap();

    let mut run = starting.started().await.unwrap();
    assert!(!run.run_id().is_empty());
    // The body is parked before its first line: nothing was published yet.
    assert_eq!(jobs.job_counts(), (1, 0));
    gate.notify_one();

    assert_eq!(run.next().await, Some(RunEvent::Log(lines(&["first"]))));
    assert_eq!(run.next().await, Some(RunEvent::Finished(Some(7))));
    assert_eq!(run.next().await, None);
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn finished_follows_every_log_and_skips_progress() {
    let jobs = JobRegistry::default();
    let starting = start_run(
        &jobs,
        JobSpec::new("test-run"),
        |ctx, completion: oneshot::Sender<&'static str>, started| async move {
            started.send(()).unwrap();
            for index in 0..50 {
                ctx.log(vec![format!("line {index}")]);
                ctx.progress(JobProgress::default());
            }
            completion.send("done").unwrap();
            Outcome::Succeeded
        },
    )
    .unwrap();
    let mut run = starting.started().await.unwrap();

    let mut seen = Vec::new();
    let finished = loop {
        match run.next().await.expect("Finished before the stream ends") {
            RunEvent::Log(LogBatch::Lines(batch)) => seen.extend(batch),
            RunEvent::Log(other) => panic!("unexpected batch {other:?}"),
            RunEvent::Finished(completion) => break completion,
        }
    };
    let expected: Vec<String> = (0..50).map(|index| format!("line {index}")).collect();
    assert_eq!(seen, expected);
    assert_eq!(finished, Some("done"));
    assert_eq!(run.next().await, None);
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn finished_without_completion_is_none() {
    let jobs = JobRegistry::default();
    let starting = start_run(
        &jobs,
        JobSpec::new("test-run"),
        |_ctx, completion: oneshot::Sender<u8>, started| async move {
            started.send(()).unwrap();
            drop(completion);
            Outcome::Cancelled
        },
    )
    .unwrap();
    let mut run = starting.started().await.unwrap();
    assert_eq!(run.next().await, Some(RunEvent::Finished(None)));
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_failing_before_start_surfaces_its_error() {
    let jobs = JobRegistry::default();
    let starting = start_run(
        &jobs,
        JobSpec::new("test-run"),
        |_ctx, _completion: oneshot::Sender<u8>, started| async move {
            drop(started);
            Outcome::Failed(AppError::new(AppErrorKind::Io, "spawn failed"))
        },
    )
    .unwrap();
    let error = starting.started().await.err().expect("not started");
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert_eq!(error.message(), "spawn failed");
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_cancelled_before_start_is_cancelled() {
    let jobs = JobRegistry::default();
    let starting = start_run(
        &jobs,
        JobSpec::new("test-run"),
        |_ctx, _completion: oneshot::Sender<u8>, _started| async move { Outcome::Cancelled },
    )
    .unwrap();
    let error = starting.started().await.err().expect("not started");
    assert_eq!(error.kind(), AppErrorKind::Cancelled);
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_key_conflicts_and_nothing_runs() {
    let jobs = JobRegistry::default();
    let key = JobKey::new("prefix:/p");
    let gate = Arc::new(Notify::new());
    let body_gate = Arc::clone(&gate);
    let first = start_run(
        &jobs,
        JobSpec::new("test-run").with_key(key.clone()),
        move |_ctx, completion: oneshot::Sender<u8>, started| async move {
            started.send(()).unwrap();
            body_gate.notified().await;
            completion.send(1).unwrap();
            Outcome::Succeeded
        },
    )
    .unwrap();
    let mut run = first.started().await.unwrap();

    let (ran_tx, mut ran_rx) = oneshot::channel::<()>();
    let error = start_run(
        &jobs,
        JobSpec::new("test-run").with_key(key),
        move |_ctx, _completion: oneshot::Sender<u8>, _started| async move {
            let _ = ran_tx.send(());
            Outcome::Succeeded
        },
    )
    .err()
    .expect("key is held");
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    // The rejected body was dropped unrun.
    assert!(ran_rx.try_recv().is_err());

    gate.notify_one();
    assert_eq!(run.next().await, Some(RunEvent::Finished(Some(1))));
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_run_is_reaped_after_the_job_finishes() {
    let jobs = JobRegistry::default();
    let gate = Arc::new(Notify::new());
    let body_gate = Arc::clone(&gate);
    let (done_tx, done_rx) = oneshot::channel::<()>();
    let starting = start_run(
        &jobs,
        JobSpec::new("test-run"),
        move |_ctx, completion: oneshot::Sender<u8>, started| async move {
            started.send(()).unwrap();
            body_gate.notified().await;
            let _ = completion.send(1);
            let _ = done_tx.send(());
            Outcome::Succeeded
        },
    )
    .unwrap();
    let run = starting.started().await.unwrap();
    drop(run);
    assert_eq!(jobs.job_counts(), (1, 0));
    gate.notify_one();
    done_rx.await.unwrap();

    // The background reaper runs once `Finished` is published; yield until it
    // did (bounded, no timed sleeps).
    let deadline = Instant::now() + Duration::from_secs(5);
    while jobs.job_counts() != (0, 0) {
        assert!(Instant::now() < deadline, "dropped run was never reaped");
        tokio::task::yield_now().await;
    }
}
