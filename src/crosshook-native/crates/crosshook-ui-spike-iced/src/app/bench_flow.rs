//! Bench state machine: READY after two presented frames, then the G5/G8 probe.
//!
//! READY: once summaries are loaded, wait for two `RedrawRequested` (frames) and print
//! `READY <ns>`. G5: emit clock samples, `BENCH_BEGIN`, run the scenario setup, then per trial
//! `armed` (target focused) and `render` (stamped in the update that applied the change; the
//! next frame presents it). G8: report the console follow state, `BENCH_BEGIN`, then the
//! replay thread waits for `GO`.

use crate::bench::{self, BenchConfig};

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Off,
    Loading,
    WaitFrames(u8),
    /// G8: waiting for the console to be visible (deadline in frames is not used; wall clock).
    WaitConsole(std::time::Instant),
    Setup,
    Armed(u64),
    Done,
}

#[derive(Debug, Clone)]
pub struct BenchFlow {
    pub phase: Phase,
    pub scenario: Option<String>,
    pub ready_done: bool,
    pub focus_lost_sent: bool,
}

pub const CLOCK_SAMPLES: u64 = 40;

impl BenchFlow {
    pub fn new(cfg: &BenchConfig) -> Self {
        Self {
            phase: if cfg.enabled {
                Phase::Loading
            } else {
                Phase::Off
            },
            scenario: cfg.scenario.clone(),
            ready_done: false,
            focus_lost_sent: false,
        }
    }

    pub fn wants_frames(&self) -> bool {
        matches!(self.phase, Phase::WaitFrames(_) | Phase::WaitConsole(_))
    }

    pub fn loaded(&mut self) {
        if self.phase == Phase::Loading {
            self.phase = Phase::WaitFrames(0);
        }
    }

    /// Returns true when READY was just printed.
    pub fn frame(&mut self) -> bool {
        if let Phase::WaitFrames(n) = self.phase {
            if n + 1 >= 2 {
                bench::emit(&bench::ready_line(bench::monotonic_ns()));
                self.ready_done = true;
                self.phase = Phase::Setup;
                return true;
            }
            self.phase = Phase::WaitFrames(n + 1);
        }
        false
    }

    pub fn clock_sync() {
        for seq in 0..CLOCK_SAMPLES {
            let t = bench::perf_us();
            bench::emit(&bench::clock_line(bench::monotonic_ns(), seq, t));
        }
    }

    pub fn begin(&self) {
        if let Some(s) = &self.scenario {
            bench::emit(&bench::begin_line(bench::monotonic_ns(), s));
        }
    }

    pub fn armed(&mut self, seq: u64) {
        if let Some(s) = self.scenario.clone() {
            bench::mark(&s, seq, "armed");
            self.phase = Phase::Armed(seq);
        }
    }

    /// Call in the update that applied the scenario's change. Returns the next seq.
    pub fn rendered(&mut self) -> Option<u64> {
        let Phase::Armed(seq) = self.phase else {
            return None;
        };
        if let Some(s) = self.scenario.clone() {
            bench::mark(&s, seq, "render");
        }
        Some(seq + 1)
    }

    /// A real blur only: the window must have been focused after READY first (as a DOM `blur`).
    pub fn focus_lost(&mut self, seen_focus: bool) {
        if self.focus_lost_sent || !self.ready_done || !seen_focus {
            return;
        }
        if let Some(s) = self.scenario.clone() {
            self.focus_lost_sent = true;
            bench::mark(&s, 0, "focus_lost");
        }
    }
}
