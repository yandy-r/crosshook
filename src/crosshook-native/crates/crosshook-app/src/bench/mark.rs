//! App-side `BENCH_BEGIN` / `BENCH_MARK` / `BENCH_CLOCK` markers.
//!
//! Every entry point checks the bench gate first and does nothing (no parse, no
//! write, no state change) unless `CROSSHOOK_BENCH=1`. Arguments are validated
//! before anything is written, and the emitted tokens come from the compiled-in
//! enums and the parsed numbers, never from echoed input. Each marker is one
//! `writeln!` under the stderr lock, so a line can neither contain a newline nor
//! interleave with another writer. Marks and clock samples are each capped per
//! process; past the cap one `BENCH_ERROR:` line is written and the rest dropped.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde::Deserialize;

use super::env::BenchEnv;
use super::scenario::{parse_scenario, Scenario};
use super::util::{claim, monotonic_ns, write_line, write_line_to};

static BEGIN_FIRED: AtomicBool = AtomicBool::new(false);
static MARKS: Budget = Budget::new();
static CLOCKS: Budget = Budget::new();

const MAX_SEQ: u32 = 1_000_000;
const MAX_T_PERF_MS: f64 = 86_400_000.0;
/// Marker writes allowed per process, per kind (mark / clock).
const MARKER_CAP: u64 = 100_000;
const CAP_REACHED_LINE: &str = "BENCH_ERROR: marker cap reached";

/// Closed phase set; the emitted token is the variant, never the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Armed,
    Render,
    FocusLost,
}

impl Phase {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Armed => "armed",
            Self::Render => "render",
            Self::FocusLost => "focus_lost",
        }
    }

    pub(super) fn parse(raw: &str) -> Option<Self> {
        [Self::Armed, Self::Render, Self::FocusLost]
            .into_iter()
            .find(|phase| phase.name() == raw)
    }
}

/// `t_perf_ms` (`performance.now()` milliseconds) as integer microseconds.
pub(super) fn t_perf_us(t_perf_ms: f64) -> Result<u64, String> {
    if !t_perf_ms.is_finite() || !(0.0..=MAX_T_PERF_MS).contains(&t_perf_ms) {
        return Err(format!(
            "t_perf_ms must be finite and within 0..={MAX_T_PERF_MS}"
        ));
    }
    // Finite and within 0..=86_400_000 ms, so the rounded microseconds fit a u64.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok((t_perf_ms * 1000.0).round() as u64)
}

fn check_seq(seq: u32) -> Result<u32, String> {
    if seq > MAX_SEQ {
        return Err(format!("seq must be at most {MAX_SEQ}"));
    }
    Ok(seq)
}

pub(super) fn format_begin(ns: u64, scenario: Scenario) -> String {
    format!("BENCH_BEGIN {ns} {}", scenario.name())
}

pub(super) fn format_mark(
    ns: u64,
    scenario: Scenario,
    seq: u32,
    phase: Phase,
    t_us: u64,
) -> String {
    format!(
        "BENCH_MARK {ns} {} {seq} {} {t_us}",
        scenario.name(),
        phase.name()
    )
}

pub(super) fn format_clock(ns: u64, seq: u32, t_us: u64) -> String {
    format!("BENCH_CLOCK {ns} {seq} {t_us}")
}

/// A fully validated mark, ready to be stamped and written.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CheckedMark {
    scenario: Scenario,
    seq: u32,
    phase: Phase,
    t_us: u64,
}

impl CheckedMark {
    pub(super) fn line(&self, ns: u64) -> String {
        format_mark(ns, self.scenario, self.seq, self.phase, self.t_us)
    }
}

/// Gate, then scenario, phase, seq and time, in that order.
/// `Ok(None)`: bench mode is off, nothing to write.
pub(super) fn check_mark(
    env: &BenchEnv,
    scenario: &str,
    seq: u32,
    phase: &str,
    t_perf_ms: f64,
) -> Result<Option<CheckedMark>, String> {
    if !env.enabled() {
        return Ok(None);
    }
    let active = parse_scenario(env)?.ok_or("no active bench scenario")?;
    if active.name() != scenario {
        return Err("scenario does not match the active bench scenario".to_string());
    }
    let phase = Phase::parse(phase).ok_or("unknown bench phase")?;
    let seq = check_seq(seq)?;
    let t_us = t_perf_us(t_perf_ms)?;
    Ok(Some(CheckedMark {
        scenario: active,
        seq,
        phase,
        t_us,
    }))
}

/// Validate the clock-sync sample. `Ok(None)`: bench mode is off.
pub(super) fn check_clock(
    env: &BenchEnv,
    seq: u32,
    t_perf_ms: f64,
) -> Result<Option<(u32, u64)>, String> {
    if !env.enabled() {
        return Ok(None);
    }
    Ok(Some((check_seq(seq)?, t_perf_us(t_perf_ms)?)))
}

/// Validate and claim the begin one-shot; the claim is taken only after every
/// check passed, so an error leaves the flag unclaimed. `Ok(None)`: disabled or
/// already fired.
pub(super) fn plan_begin(flag: &AtomicBool, env: &BenchEnv) -> Result<Option<String>, String> {
    if !env.enabled() {
        return Ok(None);
    }
    let scenario = parse_scenario(env)?.ok_or("no active bench scenario")?;
    if flag.load(Ordering::Acquire) {
        return Ok(None);
    }
    let ns = monotonic_ns()?;
    Ok(claim(flag).then(|| format_begin(ns, scenario)))
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Admit {
    Write,
    /// The first write past the cap: report it once.
    CapReached,
    Drop,
}

/// Per-process write counter.
pub(super) struct Budget(AtomicU64);

impl Budget {
    pub(super) const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub(super) fn admit(&self, cap: u64) -> Admit {
        match self.0.fetch_add(1, Ordering::AcqRel) {
            used if used < cap => Admit::Write,
            used if used == cap => Admit::CapReached,
            _ => Admit::Drop,
        }
    }
}

/// Write `line` if the budget admits it; `Ok(true)` when written.
pub(super) fn emit(
    out: &mut impl Write,
    budget: &Budget,
    cap: u64,
    line: &str,
) -> std::io::Result<bool> {
    match budget.admit(cap) {
        Admit::Write => write_line_to(out, line).map(|()| true),
        Admit::CapReached => write_line_to(out, CAP_REACHED_LINE).map(|()| false),
        Admit::Drop => Ok(false),
    }
}

/// The `ns` a clock sample reports back: the stamp when its line was written, `0` when the
/// budget dropped it (past the cap), since a dropped sample has no matching stderr line.
pub(super) fn reported_clock_ns(ns: u64, written: bool) -> u64 {
    if written {
        ns
    } else {
        0
    }
}

fn marker_write_error(error: &std::io::Error) -> String {
    format!("failed to write bench marker: {error}")
}

/// One-shot `BENCH_BEGIN <ns> <scenario>`. A no-op unless `CROSSHOOK_BENCH=1`
/// (and again after the first call).
///
/// # Errors
/// No valid active scenario, or a failed write.
pub fn bench_begin() -> Result<(), String> {
    let Some(line) = plan_begin(&BEGIN_FIRED, &BenchEnv::from_process())? else {
        return Ok(());
    };
    write_line(&line).map_err(|e| marker_write_error(&e))
}

/// `BENCH_MARK <ns> <scenario> <seq> <phase> <t_perf_us>`. A no-op unless
/// `CROSSHOOK_BENCH=1`; otherwise validates every argument before writing.
///
/// # Errors
/// A scenario that is not the active one, an unknown phase, an out-of-range
/// `seq` or `t_perf_ms`, or a failed write. Error text never echoes input.
pub fn bench_mark(scenario: &str, seq: u32, phase: &str, t_perf_ms: f64) -> Result<(), String> {
    let env = BenchEnv::from_process();
    let Some(mark) = check_mark(&env, scenario, seq, phase, t_perf_ms)? else {
        return Ok(());
    };
    let line = mark.line(monotonic_ns()?);
    emit(&mut std::io::stderr().lock(), &MARKS, MARKER_CAP, &line)
        .map(|_| ())
        .map_err(|e| marker_write_error(&e))
}

/// `BENCH_CLOCK <ns> <seq> <t_perf_us>`: one clock-sync sample. Returns the
/// monotonic `ns` it stamped. A no-op returning `Ok(0)` unless `CROSSHOOK_BENCH=1`
/// (also `Ok(0)` for a sample dropped past the cap).
///
/// # Errors
/// An out-of-range `seq` or `t_perf_ms`, or a failed write.
pub fn bench_clock(seq: u32, t_perf_ms: f64) -> Result<u64, String> {
    let env = BenchEnv::from_process();
    let Some((seq, t_us)) = check_clock(&env, seq, t_perf_ms)? else {
        return Ok(0);
    };
    let ns = monotonic_ns()?;
    let line = format_clock(ns, seq, t_us);
    emit(&mut std::io::stderr().lock(), &CLOCKS, MARKER_CAP, &line)
        .map(|written| reported_clock_ns(ns, written))
        .map_err(|e| marker_write_error(&e))
}

#[cfg(test)]
mod tests;
