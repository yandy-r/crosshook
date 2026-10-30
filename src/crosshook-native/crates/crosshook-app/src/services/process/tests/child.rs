//! `run_child`: live streaming, final-drain ordering, lossy flood, bounded
//! drain past a pipe-holding descendant (kept through a cooperative cancel,
//! cut short by the kill token), kill token, `started` only after a spawn.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::jobs::{create_log_file_in, JobContext, JobId, JobRegistry, JobSpec, Outcome};
use crate::log_tail::LogBatch;
use crate::services::process::child::{
    exit_code_and_signal, run_child, ChildError, ChildOutput, LogFile, DRAIN_GRACE,
};
use crate::services::process::run::{start_run, StartingRun};
use crate::services::process::{ProcessRun, RunEvent};
use crate::{AppError, AppErrorKind};

/// `(code, signal)` of the child, or the `run_child` error text.
type Completion = Result<(Option<i32>, Option<i32>), String>;

/// Starts a job whose body runs `sh -c script args…` through `run_child`,
/// relaying the tail into the job log; the completion is the exit's
/// `(code, signal)`. Returns the run (already started) and the log path.
async fn start_sh(
    jobs: &JobRegistry,
    dir: &Path,
    script: &str,
    args: &[&str],
    output: ChildOutput,
    kill: CancellationToken,
) -> (ProcessRun<Completion>, PathBuf) {
    let (path, file) = create_log_file_in(dir.to_path_buf(), "test", "child").unwrap();
    let log_path = path.clone();
    let mut command = Command::new("sh");
    command.arg("-c").arg(script).args(args);
    let starting = start_run(
        jobs,
        JobSpec::new("test-child"),
        move |ctx, completion: oneshot::Sender<Completion>, started| async move {
            let sink_ctx = ctx.clone();
            let result = run_child(
                &ctx,
                command,
                LogFile { path, file },
                output,
                &kill,
                started,
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
    make_named_fifo(dir, "gate")
}

fn make_named_fifo(dir: &Path, name: &str) -> PathBuf {
    let fifo = dir.join(name);
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
        &[fifo.to_str().unwrap()],
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
        &["flood"],
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
        &[pid_file.to_str().unwrap()],
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
async fn kill_abandons_a_held_pipe_without_waiting_out_the_grace() {
    let dir = TempDir::new().unwrap();
    let pid_file = dir.path().join("descendant.pid");
    let jobs = JobRegistry::default();
    let kill = CancellationToken::new();
    let (mut run, _) = start_sh(
        &jobs,
        dir.path(),
        "sleep 30 & echo $! > \"$0\"; echo done",
        &[pid_file.to_str().unwrap()],
        ChildOutput::Piped,
        kill.clone(),
    )
    .await;
    assert_eq!(next_line(&mut run).await, "done");
    let killed = Instant::now();
    kill.cancel();

    let (_, completion) = drain(&mut run).await;
    let elapsed = killed.elapsed();
    kill_pid(&pid_file);
    assert!(completion.is_some_and(|result| result.is_ok()));
    assert!(
        elapsed < DRAIN_GRACE,
        "kill waited out the grace: {elapsed:?}"
    );
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cooperative_cancel_still_drains_output_written_after_the_leader_exited() {
    let dir = TempDir::new().unwrap();
    let exiting = make_named_fifo(dir.path(), "exiting");
    let pid_file = dir.path().join("descendant.pid");
    let jobs = JobRegistry::default();
    // A detached descendant (own session, so the group SIGTERM misses it)
    // holds both pipes. The leader announces its SIGTERM exit on `exiting`;
    // only then — after a margin far shorter than DRAIN_GRACE but far
    // longer than the supervisor needs to notice the exit — does the
    // descendant write its last line and close the pipes.
    let script =
        "setsid sh -c 'echo $$ > \"$1\"; read x < \"$0\"; sleep 0.5; echo late' \"$0\" \"$1\" &\n\
                  trap 'echo x > \"$0\"; exit 0' TERM\n\
                  echo ready\n\
                  sleep 30 &\n\
                  wait";
    let (mut run, _) = start_sh(
        &jobs,
        dir.path(),
        script,
        &[exiting.to_str().unwrap(), pid_file.to_str().unwrap()],
        ChildOutput::Piped,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(next_line(&mut run).await, "ready");

    jobs.cancel(&JobId::from_string(run.run_id())).unwrap();
    let (lines, completion) = drain(&mut run).await;
    kill_pid(&pid_file);
    assert_eq!(
        lines,
        vec!["late".to_string()],
        "a cooperative cancel must not drop output still in the pipes"
    );
    assert_eq!(completion, Some(Ok((Some(0), None))));
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
        &["kill"],
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

/// A future run by a test body before it reaches `run_child`.
type Before = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Starts a job whose body awaits `before`, runs `command` through
/// `run_child` and ends with the `run_child` error as its outcome.
fn start_failing(
    jobs: &JobRegistry,
    dir: &Path,
    command: Command,
    before: impl FnOnce(JobContext) -> Before + Send + 'static,
) -> StartingRun<()> {
    let (path, file) = create_log_file_in(dir.to_path_buf(), "test", "child").unwrap();
    start_run(
        jobs,
        JobSpec::new("test-child"),
        move |ctx, _completion: oneshot::Sender<()>, started| async move {
            before(ctx.clone()).await;
            let result = run_child(
                &ctx,
                command,
                LogFile { path, file },
                ChildOutput::Piped,
                &CancellationToken::new(),
                started,
                |_| panic!("no output expected"),
            )
            .await;
            match result {
                Ok(_) => Outcome::Succeeded,
                Err(ChildError::Cancelled) => Outcome::Cancelled,
                Err(ChildError::Spawn(error)) => Outcome::Failed(AppError::new(
                    AppErrorKind::Io,
                    format!("spawn {:?}", error.kind()),
                )),
                Err(ChildError::Supervise(error)) => {
                    Outcome::Failed(AppError::new(AppErrorKind::Internal, error.to_string()))
                }
            }
        },
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_failure_fails_the_start_and_nothing_streams() {
    let dir = TempDir::new().unwrap();
    let jobs = JobRegistry::default();
    let starting = start_failing(
        &jobs,
        dir.path(),
        Command::new(dir.path().join("no-such-program")),
        |_| Box::pin(async {}),
    );

    // `started` was never sent: the start itself reports the spawn error.
    let error = starting.started().await.err().expect("spawn failed");
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert_eq!(error.message(), "spawn NotFound");
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_job_cancelled_before_the_spawn_runs_nothing() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("ran");
    let jobs = JobRegistry::default();
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg("touch \"$0\"")
        .arg(marker.to_str().unwrap());
    let (id_tx, id_rx) = oneshot::channel();
    let starting = start_failing(&jobs, dir.path(), command, move |ctx| {
        Box::pin(async move {
            let _ = id_tx.send(ctx.id().clone());
            ctx.cancelled().await;
        })
    });
    jobs.cancel(&id_rx.await.unwrap()).unwrap();

    let error = starting.started().await.err().expect("cancelled");
    assert_eq!(error.kind(), AppErrorKind::Cancelled);
    assert!(!marker.exists(), "nothing may be spawned after a cancel");
    assert_eq!(jobs.job_counts(), (0, 0));
}
