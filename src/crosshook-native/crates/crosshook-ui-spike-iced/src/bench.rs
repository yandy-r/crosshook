//! Bench-only stderr markers (`CROSSHOOK_BENCH=1`), matching `scripts/bench/bench_events.py`.
//!
//! Every `<ns>` is CLOCK_MONOTONIC. `t_perf_us` is CLOCK_MONOTONIC microseconds since process
//! start, so the runner's clock sync between `BENCH_CLOCK` samples is exact (no IPC hop).

use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

pub const MAX_SEQ: u64 = 1_000_000;
pub const MAX_MARKS: u32 = 100_000;

/// G5 scenarios plus the G8 replay scenario this spike supports.
pub const SCENARIOS: &[&str] = &[
    "search",
    "toggle_favorite",
    "toggle_optimization",
    "dpad",
    "route_switch",
    "replay_scroll",
];

static START_NS: OnceLock<u64> = OnceLock::new();
static MARKS: AtomicU32 = AtomicU32::new(0);

pub fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid pointer to a timespec; CLOCK_MONOTONIC is always available on Linux.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Call first thing in `main` so `perf_us` starts near zero.
pub fn init_clock() {
    let _ = START_NS.set(monotonic_ns());
}

pub fn perf_us() -> u64 {
    let start = *START_NS.get_or_init(monotonic_ns);
    monotonic_ns().saturating_sub(start) / 1000
}

#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub enabled: bool,
    pub exit_after_ready: bool,
    pub scenario: Option<String>,
    pub log: Option<std::path::PathBuf>,
    pub rate: u32,
}

impl BenchConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("CROSSHOOK_BENCH").as_deref() == Ok("1");
        let scenario = std::env::var("CROSSHOOK_BENCH_SCENARIO")
            .ok()
            .filter(|s| !s.is_empty());
        let scenario = match scenario {
            Some(s) if enabled && !SCENARIOS.contains(&s.as_str()) => {
                emit(&format!("BENCH_ERROR: unknown scenario {s:?}"));
                None
            }
            other if enabled => other,
            _ => None,
        };
        let rate = std::env::var("CROSSHOOK_BENCH_LOG_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000u32)
            .clamp(1, 5000);
        Self {
            enabled,
            exit_after_ready: enabled
                && std::env::var("CROSSHOOK_BENCH_EXIT_AFTER_READY").as_deref() == Ok("1"),
            scenario,
            log: std::env::var_os("CROSSHOOK_BENCH_LOG")
                .map(Into::into)
                .filter(|_| enabled),
            rate,
        }
    }

    pub fn is_g5(&self) -> bool {
        self.scenario
            .as_deref()
            .is_some_and(|s| s != "replay_scroll")
    }

    pub fn is_g8(&self) -> bool {
        self.scenario.as_deref() == Some("replay_scroll")
    }
}

pub fn emit(line: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{line}");
    let _ = err.flush();
}

pub fn ready_line(ns: u64) -> String {
    format!("READY {ns}")
}

pub fn clock_line(ns: u64, seq: u64, t_us: u64) -> String {
    format!("BENCH_CLOCK {ns} {seq} {t_us}")
}

pub fn begin_line(ns: u64, scenario: &str) -> String {
    format!("BENCH_BEGIN {ns} {scenario}")
}

pub fn mark_line(ns: u64, scenario: &str, seq: u64, phase: &str, t_us: u64) -> String {
    format!("BENCH_MARK {ns} {scenario} {seq} {phase} {t_us}")
}

pub fn replay_line(ns: u64, kind: &str, lines: usize) -> String {
    format!("BENCH_REPLAY {ns} {kind} {lines}")
}

/// Emits a mark, honouring the seq bound and the per-process cap.
pub fn mark(scenario: &str, seq: u64, phase: &str) {
    if seq > MAX_SEQ {
        return;
    }
    let n = MARKS.fetch_add(1, Ordering::Relaxed);
    if n == MAX_MARKS {
        emit("BENCH_ERROR: marker cap reached");
    }
    if n >= MAX_MARKS {
        return;
    }
    let t = perf_us();
    emit(&mark_line(monotonic_ns(), scenario, seq, phase, t));
}

/// Blocks (on a plain thread) for exactly `GO\n` on stdin within `timeout`, using poll(2).
pub fn wait_go(timeout: std::time::Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    let mut got = Vec::with_capacity(3);
    while got.len() < 3 {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err("timeout".into());
        }
        let mut pfd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for stdin.
        let r = unsafe { libc::poll(&mut pfd, 1, left.as_millis().min(i32::MAX as u128) as i32) };
        if r < 0 {
            return Err("poll failed".into());
        }
        if r == 0 {
            continue;
        }
        let mut byte = 0u8;
        // SAFETY: reading one byte into a valid buffer.
        let n = unsafe { libc::read(0, (&raw mut byte).cast(), 1) };
        if n <= 0 {
            return Err("stdin closed".into());
        }
        got.push(byte);
    }
    if got == b"GO\n" {
        Ok(())
    } else {
        Err("unexpected bytes".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_match_runner_field_counts() {
        assert_eq!(ready_line(5), "READY 5");
        assert_eq!(clock_line(1, 2, 3).split(' ').count(), 4);
        assert_eq!(begin_line(1, "dpad").split(' ').count(), 3);
        assert_eq!(mark_line(1, "dpad", 0, "armed", 9).split(' ').count(), 6);
        assert_eq!(
            replay_line(1, "start", 50_000),
            "BENCH_REPLAY 1 start 50000"
        );
    }

    #[test]
    fn perf_clock_is_monotonic() {
        init_clock();
        let a = perf_us();
        let b = perf_us();
        assert!(b >= a);
    }
}
