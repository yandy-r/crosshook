//! `run_child`: live streaming, final-drain ordering, lossy flood, bounded
//! drain past a pipe-holding descendant, kill token, spawn failure.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::jobs::{create_log_file_in, JobId, JobRegistry, JobSpec, Outcome};
use crate::log_tail::LogBatch;
use crate::services::process::child::{
    exit_code_and_signal, run_child, ChildOutput, LogFile, DRAIN_GRACE,
};
use crate::services::process::run::start_run;
use crate::services::process::{ProcessRun, RunEvent};

/// `(code, signal)` of the child, or the `run_child` error text.
type Completion = Result<(Option<i32>, Option<i32>), String>;

/// Starts a job whose body runs `sh -c script arg0` through `run_child`,
/// relaying the tail into the job log; the completion is the exit's
/// `(code, signal)`. Returns the run (already started) and the log path.
async fn start_sh(
    jobs: &JobRegistry,
    dir: &Path,
    script: &str,
    arg0: &str,
    output: ChildOutput,
    kill: CancellationToken,
) -> (ProcessRun<Completion>, PathBuf) {
    let (path, file) = create_log_file_in(dir.to_path_buf(), "test", "child").unwrap();
    let log_path = path.clone();
    let mut command = Command::new("sh");
    command.arg("-c").arg(script).arg(arg0);
    let starting = start_run(
        jobs,
        JobSpec::new("test-child"),
        move |ctx, completion: oneshot::Sender<Completion>, started| async move {
            started.send(()).unwrap();
            let sink_ctx = ctx.clone();
            let result = run_child(
                &ctx,
                command,
                LogFile { path, file },
                output,
                &kill,
                move |batch| sink_ctx.log_batch(batch),
            )
            .await;
            let completion_value = result
                .as_ref()
                .map(exit_code_and_signal)
                .map_err(ToString::to_string);
            let _ = completion.send(completion_value);
            match result {
                Ok(exit) if exit.was_cancelled() => Outcome::Cancelled,
                _ => Outcome::Succeeded,
            }
        },
    )
    .unwrap();
    (starting.started().await.unwrap(), log_path)
}

/// Collects log lines until `Finished`; returns them with the completion.
async fn drain(run: &mut ProcessRun<Completion>) -> (Vec<String>, Option<Completion>) {
    let mut lines = Vec::new();
    loop {
        match run.next().await.expect("Finished before the stream ends") {
            RunEvent::Log(LogBatch::Lines(batch)) => lines.extend(batch),
            RunEvent::Log(_) => {}
            RunEvent::Finished(completion) => return (lines, completion),
        }
    }
}

/// Next log line, skipping notices; panics when the run finishes first.
async fn next_line(run: &mut ProcessRun<Completion>) -> String {
    loop {
        match run.next().await.expect("run ended") {
            RunEvent::Log(LogBatch::Lines(mut batch)) if !batch.is_empty() => {
                return batch.remove(0)
            }
            RunEvent::Log(_) => {}
            RunEvent::Finished(_) => panic!("run finished before the expected line"),
        }
    }
}

fn make_fifo(dir: &Path) -> PathBuf {
    let fifo = dir.join("gate");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    fifo
}

/// Opens the gate the child blocks on (open-for-write rendezvous).
async fn open_gate(fifo: PathBuf) {
    tokio::task::spawn_blocking(move || std::fs::write(fifo, b"go\n"))
        .await
        .unwrap()
        .unwrap();
}

async fn lines_stream_live_then_finish(output: ChildOutput) {
    let dir = TempDir::new().unwrap();
    let fifo = make_fifo(dir.path());
    let jobs = JobRegistry::default();
    let (mut run, log_path) = start_sh(
        &jobs,
        dir.path(),
        // stdout, then after the gate stderr: the gate fixes the order even
        // though the two pipes are drained by separate readers.
        "echo one; read x < \"$0\"; echo two >&2",
        fifo.to_str().unwrap(),
        output,
        CancellationToken::new(),
    )
    .await;

    // The child is blocked on the gate: this line arrived before exit.
    assert_eq!(next_line(&mut run).await, "one");
    open_gate(fifo).await;
    let (rest, completion) = drain(&mut run).await;
    assert_eq!(rest, vec!["two".to_string()]);
    assert_eq!(completion, Some(Ok((Some(0), None))));
    assert_eq!(run.next().await, None);
    assert_eq!(std::fs::read_to_string(log_path).unwrap(), "one\ntwo\n");
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn piped_lines_arrive_before_exit_and_finished_comes_last() {
    lines_stream_live_then_finish(ChildOutput::Piped).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_file_lines_arrive_before_exit_and_finished_comes_last() {
    lines_stream_live_then_finish(ChildOutput::LogFile).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_utf8_and_newline_free_flood_do_not_stall() {
    let dir = TempDir::new().unwrap();
    let jobs = JobRegistry::default();
    // Octal escapes: POSIX printf (dash) has no `\x`.
    let (mut run, log_path) = start_sh(
        &jobs,
        dir.path(),
        "printf 'bad\\377\\376\\n'; head -c 1048576 /dev/zero | tr '\\0' 'A'; printf '\\nend\\n'",
        "flood",
        ChildOutput::Piped,
        CancellationToken::new(),
    )
    .await;

    let (lines, completion) = drain(&mut run).await;
    assert_eq!(completion, Some(Ok((Some(0), None))));
    assert_eq!(
        lines.first().map(String::as_str),
        Some("bad\u{FFFD}\u{FFFD}")
    );
    assert_eq!(lines.last().map(String::as_str), Some("end"));
    let log = std::fs::read(log_path).unwrap();
    assert_eq!(log.len(), 6 + 1_048_576 + 1 + 4, "every byte was teed");
    assert_eq!(jobs.job_counts(), (0, 0));
}

fn kill_pid(pid_file: &Path) {
    if let Ok(text) = std::fs::read_to_string(pid_file) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn descendant_holding_the_pipe_is_abandoned_after_drain_grace() {
    let dir = TempDir::new().unwrap();
    let pid_file = dir.path().join("descendant.pid");
    let jobs = JobRegistry::default();
    let started = Instant::now();
    // The leader exits at once; its background child inherits and holds both
    // pipes, so they never reach EOF on their own.
    let (mut run, _) = start_sh(
        &jobs,
        dir.path(),
        "sleep 30 & echo $! > \"$0\"; echo done",
        pid_file.to_str().unwrap(),
        ChildOutput::Piped,
        CancellationToken::new(),
    )
    .await;

    let (lines, completion) = drain(&mut run).await;
    let elapsed = started.elapsed();
    kill_pid(&pid_file);
    assert_eq!(lines, vec!["done".to_string()]);
    assert_eq!(completion, Some(Ok((Some(0), None))));
    assert!(
        elapsed < DRAIN_GRACE + Duration::from_secs(3),
        "drain was not bounded: {elapsed:?}"
    );
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_abandons_a_held_pipe_without_waiting_out_the_grace() {
    let dir = TempDir::new().unwrap();
    let pid_file = dir.path().join("descendant.pid");
    let jobs = JobRegistry::default();
    let (mut run, _) = start_sh(
        &jobs,
        dir.path(),
        "sleep 30 & echo $! > \"$0\"; echo done",
        pid_file.to_str().unwrap(),
        ChildOutput::Piped,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(next_line(&mut run).await, "done");
    let cancelled = Instant::now();
    jobs.cancel(&JobId::from_string(run.run_id())).unwrap();

    let (_, completion) = drain(&mut run).await;
    let elapsed = cancelled.elapsed();
    kill_pid(&pid_file);
    assert!(completion.is_some_and(|result| result.is_ok()));
    assert!(
        elapsed < DRAIN_GRACE,
        "cancel waited out the grace: {elapsed:?}"
    );
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_token_sigkills_the_group() {
    let dir = TempDir::new().unwrap();
    let jobs = JobRegistry::default();
    let kill = CancellationToken::new();
    let (mut run, _) = start_sh(
        &jobs,
        dir.path(),
        "trap '' TERM; echo ready; exec sleep 30",
        "kill",
        ChildOutput::Piped,
        kill.clone(),
    )
    .await;
    assert_eq!(next_line(&mut run).await, "ready");
    kill.cancel();

    let (_, completion) = drain(&mut run).await;
    assert_eq!(completion, Some(Ok((None, Some(9)))));
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_failure_is_an_error_and_nothing_streams() {
    let dir = TempDir::new().unwrap();
    let (path, file) = create_log_file_in(dir.path().to_path_buf(), "test", "missing").unwrap();
    let missing = dir.path().join("no-such-program");
    let jobs = JobRegistry::default();
    let starting = start_run(
        &jobs,
        JobSpec::new("test-child"),
        move |ctx, completion: oneshot::Sender<Completion>, started| async move {
            started.send(()).unwrap();
            let result = run_child(
                &ctx,
                Command::new(missing),
                LogFile { path, file },
                ChildOutput::Piped,
                &CancellationToken::new(),
                |_| panic!("no output expected"),
            )
            .await;
            let _ = completion.send(
                result
                    .as_ref()
                    .map(exit_code_and_signal)
                    .map_err(|error| format!("{:?}", error.kind())),
            );
            Outcome::Succeeded
        },
    )
    .unwrap();
    let mut run = starting.started().await.unwrap();
    let (lines, completion) = drain(&mut run).await;
    assert!(lines.is_empty());
    assert_eq!(completion, Some(Err("NotFound".to_string())));
}
