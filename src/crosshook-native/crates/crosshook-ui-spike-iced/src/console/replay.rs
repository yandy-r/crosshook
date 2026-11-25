//! Log replay into the console: a plain reader thread paces lines at the requested rate and
//! coalesces them into one batch per ~frame (16 ms), so `update()` sees at most ~60 messages/s
//! however fast the log arrives. Exposed as a `Subscription::run_with` keyed by the plan.

use std::hash::Hash;
use std::io::BufRead;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::futures::SinkExt;
use iced::{Subscription, stream};

use crate::bench;

pub const BATCH_INTERVAL: Duration = Duration::from_millis(16);
const MAX_LINES: usize = 50_000;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ReplayPlan {
    pub path: PathBuf,
    pub rate: u32,
    /// G8: wait for `GO\n` on stdin before the first line.
    pub gated: bool,
    /// Emit `BENCH_REPLAY` markers (G8 only).
    pub markers: bool,
}

#[derive(Debug, Clone)]
pub enum ReplayEvent {
    Batch(Vec<String>),
    Done { sent: usize },
    Failed(String),
}

pub fn subscription(plan: ReplayPlan) -> Subscription<ReplayEvent> {
    Subscription::run_with(plan, |plan| {
        let plan = plan.clone();
        stream::channel(64, async move |mut out| {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            std::thread::Builder::new()
                .name("spike-replay".into())
                .spawn(move || run(&plan, &tx))
                .expect("spawn replay thread");
            while let Some(event) = rx.recv().await {
                if out.send(event).await.is_err() {
                    break;
                }
            }
        })
    })
}

fn run(plan: &ReplayPlan, tx: &tokio::sync::mpsc::UnboundedSender<ReplayEvent>) {
    let lines = match read_lines(&plan.path) {
        Ok(lines) => lines,
        Err(error) => {
            let _ = tx.send(ReplayEvent::Failed(error));
            return;
        }
    };
    if plan.gated
        && let Err(reason) = bench::wait_go(Duration::from_secs(120))
    {
        bench::emit(&format!("BENCH_ERROR: GO_SIGNAL_FAILED: {reason}"));
        let _ = tx.send(ReplayEvent::Failed(reason));
        return;
    }
    if plan.markers {
        bench::emit(&bench::replay_line(
            bench::monotonic_ns(),
            "start",
            lines.len(),
        ));
    }
    let start = Instant::now();
    let per_line = Duration::from_secs_f64(1.0 / f64::from(plan.rate.max(1)));
    let mut sent = 0usize;
    let mut batch = Vec::new();
    let mut last_flush = Instant::now();
    for line in lines {
        let due = start + per_line * sent as u32;
        let now = Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        }
        batch.push(line);
        sent += 1;
        if last_flush.elapsed() >= BATCH_INTERVAL {
            if tx
                .send(ReplayEvent::Batch(std::mem::take(&mut batch)))
                .is_err()
            {
                return;
            }
            last_flush = Instant::now();
        }
    }
    if !batch.is_empty() {
        let _ = tx.send(ReplayEvent::Batch(batch));
    }
    if plan.markers {
        bench::emit(&bench::replay_line(bench::monotonic_ns(), "done", sent));
    }
    let _ = tx.send(ReplayEvent::Done { sent });
}

fn read_lines(path: &PathBuf) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open log: {e}"))?;
    std::io::BufReader::new(file)
        .lines()
        .take(MAX_LINES)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("read log: {e}"))
}
