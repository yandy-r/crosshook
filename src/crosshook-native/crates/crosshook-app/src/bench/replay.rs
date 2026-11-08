//! Paced replay of a validated log: a fixed number of lines per second,
//! handed to the shell as `launch-log` wire events tagged with the `bench`
//! session.

use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;

use super::plan::{ReplayPlan, MAX_REPLAY_BYTES};
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

/// Replays the plan's log at its rate, calling `emit` for each line's
/// `launch-log` event. Stops early when `emit` returns `false` (the shell
/// could not deliver the event).
pub async fn replay_log(plan: ReplayPlan, mut emit: impl FnMut(WireEvent) -> bool) {
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
    tracing::info!(lines = sent, "bench log replay complete");
}

#[cfg(test)]
mod tests;
