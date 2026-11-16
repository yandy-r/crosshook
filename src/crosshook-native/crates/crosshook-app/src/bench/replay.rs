//! Paced replay of a validated log: a fixed number of lines per second,
//! handed to the shell as `launch-log` wire events tagged with the `bench`
//! session. A replay the runner starts with its go line also writes `BENCH_REPLAY` start and done
//! markers on stderr; the replay READY starts (G4) writes none.

use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;

use super::env::GO_TIMEOUT_S;
use super::go::await_go;
use super::plan::{ReplayPlan, MAX_REPLAY_BYTES};
use super::util::{monotonic_ns, write_line};
use crate::ipc::launch::payloads::LaunchLogPayload;
use crate::ipc::launch::LAUNCH_LOG_EVENT;
use crate::ipc::WireEvent;

const MAX_REPLAY_LINES: usize = 50_000;
const TICK: Duration = Duration::from_millis(10);
const BENCH_SESSION_ID: &str = "bench";

/// Lines due after `elapsed_ms` at `rate` lines per second, capped at `total`.
fn due_lines(rate: u32, elapsed_ms: u128, total: usize) -> usize {
    usize::try_from(u128::from(rate).saturating_mul(elapsed_ms) / 1000)
        .unwrap_or(usize::MAX)
        .min(total)
}

/// The replayable lines of `text`: non-empty, at most [`MAX_REPLAY_LINES`].
fn replay_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.is_empty())
        .take(MAX_REPLAY_LINES)
        .collect()
}

/// The `launch-log` event replaying `line`.
fn line_event(line: &str) -> Option<WireEvent> {
    WireEvent::new(
        LAUNCH_LOG_EVENT,
        &LaunchLogPayload {
            line,
            session_id: BENCH_SESSION_ID,
        },
    )
}

/// `BENCH_REPLAY <ns> <start|done> <lines>`: `start` carries the line total, `done` the lines sent.
fn format_replay_marker(ns: u64, kind: &str, lines: usize) -> String {
    format!("BENCH_REPLAY {ns} {kind} {lines}")
}

/// The line reported when the go signal did not arrive.
fn go_failure_line(error: &str) -> String {
    format!("BENCH_ERROR: GO_SIGNAL_FAILED: {error}")
}

/// Write one marker line through `sink`, stamped now; a clock failure is reported as `BENCH_ERROR:`.
fn write_marker(sink: &mut impl FnMut(&str), kind: &str, lines: usize) {
    match monotonic_ns() {
        Ok(ns) => sink(&format_replay_marker(ns, kind, lines)),
        Err(error) => sink(&format!("BENCH_ERROR: {error}")),
    }
}

/// The production sink: one stderr line, a failed write is logged only.
fn write_stderr(line: &str) {
    if let Err(error) = write_line(line) {
        tracing::warn!(%error, "bench log replay: marker write failed");
    }
}

/// Replays the plan's log at its rate, calling `emit` for each line's
/// `launch-log` event. Stops early when `emit` returns `false` (the shell
/// could not deliver the event). Writes no `BENCH_REPLAY` markers: the replay
/// READY starts must stay as it is.
pub async fn replay_log(plan: ReplayPlan, emit: impl FnMut(WireEvent) -> bool) {
    run_replay(plan, emit, false, write_stderr).await;
}

/// Waits for the runner's `GO\n` line on stdin (at most [`GO_TIMEOUT_S`] seconds), then replays
/// like [`replay_log`] but bracketed by `BENCH_REPLAY <ns> start <total>` and
/// `BENCH_REPLAY <ns> done <sent>`. Without the line (EOF, timeout, other bytes) it writes
/// `BENCH_ERROR: GO_SIGNAL_FAILED: <reason>` and replays nothing.
pub async fn replay_after_go(plan: ReplayPlan, emit: impl FnMut(WireEvent) -> bool) {
    let go = await_go(Duration::from_secs(GO_TIMEOUT_S)).await;
    after_go(plan, go, emit, write_stderr).await;
}

async fn after_go(
    plan: ReplayPlan,
    go: Result<(), String>,
    emit: impl FnMut(WireEvent) -> bool,
    mut sink: impl FnMut(&str),
) {
    match go {
        Ok(()) => run_replay(plan, emit, true, sink).await,
        Err(error) => sink(&go_failure_line(&error)),
    }
}

/// The replay itself. With `markers`, `start` is written after the log is read and before the
/// first emit, `done` after the last line; neither when the read fails or `emit` stops the replay.
async fn run_replay(
    plan: ReplayPlan,
    mut emit: impl FnMut(WireEvent) -> bool,
    markers: bool,
    mut sink: impl FnMut(&str),
) {
    let mut buf = Vec::new();
    let read = async {
        let file = tokio::fs::File::from_std(plan.file);
        file.take(MAX_REPLAY_BYTES).read_to_end(&mut buf).await
    };
    if let Err(error) = read.await {
        tracing::warn!(%error, log = %plan.log.display(), "bench log replay: read failed");
        return;
    }
    let text = String::from_utf8_lossy(&buf);
    let lines = replay_lines(&text);

    if markers {
        write_marker(&mut sink, "start", lines.len());
    }
    let start = Instant::now();
    let mut ticker = tokio::time::interval(TICK);
    let mut sent = 0usize;
    while sent < lines.len() {
        ticker.tick().await;
        let due = due_lines(plan.rate, start.elapsed().as_millis(), lines.len());
        for line in &lines[sent..due] {
            let Some(event) = line_event(line) else {
                continue;
            };
            if !emit(event) {
                tracing::warn!("bench log replay: emit failed; stopping");
                return;
            }
        }
        sent = sent.max(due);
    }
    if markers {
        write_marker(&mut sink, "done", sent);
    }
    tracing::info!(lines = sent, "bench log replay complete");
}

#[cfg(test)]
mod tests;
