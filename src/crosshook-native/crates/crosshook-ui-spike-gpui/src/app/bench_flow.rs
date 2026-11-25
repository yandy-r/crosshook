//! Bench protocol (same markers and phases as the iced spike's `bench_flow` + `probe`).
//!
//! READY: once summaries are loaded, wait for two frames gpui drew (`Window::on_next_frame`
//! scheduled from render) and print `READY <ns>`. G5: clock samples, `BENCH_BEGIN`, scenario
//! setup, then per trial `armed` (target focused) and `render` (stamped in the handler that
//! applied the change; the next frame presents it). G8: report the console follow state,
//! `BENCH_BEGIN`, then the replay thread waits for `GO`.

use std::time::{Duration, Instant};

use gpui::{Context, Window};

use super::{Route, Shell};
use crate::bench::{self, BenchConfig};
use crate::focus::Target;

const CONSOLE_WAIT: Duration = Duration::from_secs(30);
pub const CLOCK_SAMPLES: u64 = 40;

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Off,
    Loading,
    WaitFrames(u8),
    WaitConsole(Instant),
    Armed(u64),
    Done,
}

#[derive(Debug, Clone)]
pub struct BenchFlow {
    pub phase: Phase,
    pub scenario: Option<String>,
    pub ready_done: bool,
    pub focus_lost_sent: bool,
    frame_scheduled: bool,
}

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
            frame_scheduled: false,
        }
    }

    pub fn loaded(&mut self) {
        if self.phase == Phase::Loading {
            self.phase = Phase::WaitFrames(0);
        }
    }

    pub fn armed(&mut self, seq: u64) {
        if let Some(s) = self.scenario.clone() {
            bench::mark(&s, seq, "armed");
            self.phase = Phase::Armed(seq);
        }
    }

    /// A real blur only: the window must have been active after READY first.
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

impl Shell {
    /// Called from render: drives READY and the G8 console gate off frames gpui drew, and
    /// re-arms the next G5 trial once the change frame has been built.
    pub fn bench_after_render(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.rearm.take()
            && next <= bench::MAX_SEQ
        {
            let this = cx.entity();
            window.on_next_frame(move |window, cx| {
                this.update(cx, |this, cx| this.arm(next, window, cx));
            });
        }
        let wants = matches!(
            self.flow.phase,
            Phase::WaitFrames(_) | Phase::WaitConsole(_)
        );
        if !wants || self.flow.frame_scheduled {
            return;
        }
        self.flow.frame_scheduled = true;
        let this = cx.entity();
        window.on_next_frame(move |window, cx| {
            this.update(cx, |this, cx| {
                this.flow.frame_scheduled = false;
                this.on_bench_frame(window, cx);
                cx.notify();
            });
        });
    }

    fn on_bench_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.flow.phase {
            Phase::WaitFrames(n) if n + 1 >= 2 => {
                bench::emit(&bench::ready_line(bench::monotonic_ns()));
                self.flow.ready_done = true;
                self.after_ready(window, cx);
            }
            Phase::WaitFrames(n) => self.flow.phase = Phase::WaitFrames(n + 1),
            Phase::WaitConsole(since) => self.g8_poll(since, window, cx),
            _ => {}
        }
    }

    fn after_ready(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.bench.exit_after_ready {
            cx.quit();
            return;
        }
        if self.bench.is_g8() {
            self.flow.phase = Phase::WaitConsole(Instant::now());
            return;
        }
        if !self.bench.is_g5() {
            self.flow.phase = Phase::Done;
            self.start_replay(false, cx);
            return;
        }
        for seq in 0..CLOCK_SAMPLES {
            let t = bench::perf_us();
            bench::emit(&bench::clock_line(bench::monotonic_ns(), seq, t));
        }
        if let Some(s) = &self.flow.scenario {
            bench::emit(&bench::begin_line(bench::monotonic_ns(), s));
        }
        if self.flow.scenario.as_deref() == Some("toggle_optimization") {
            self.history.push(Route::Settings);
            // The settings page must render before its switch can take focus.
            self.rearm = Some(0);
            self.flow.phase = Phase::Done;
            cx.notify();
            return;
        }
        self.arm(0, window, cx);
    }

    /// Focuses the scenario target and sends `armed`.
    pub fn arm(&mut self, seq: u64, window: &mut Window, cx: &mut Context<Self>) {
        let target = match self.flow.scenario.as_deref() {
            Some("search") => Target::Search,
            Some("toggle_favorite") => Target::Favorite(0),
            Some("toggle_optimization") => Target::Field(12),
            Some("dpad") => Target::Card(0),
            // Focus the non-active tab without activating it.
            Some("route_switch") => Target::Nav(if self.route() == Route::Library { 1 } else { 0 }),
            _ => return,
        };
        let keep = self.flow.scenario.as_deref() == Some("dpad")
            && matches!(self.focus.focused, Some(Target::Card(_)));
        if !keep {
            self.focus_to(target, window, cx);
        }
        self.flow.armed(seq);
        cx.notify();
    }

    /// Called by every handler that applies a scenario's change: stamps `render` (this update
    /// is what the next frame presents) and schedules the next trial.
    pub fn bench_rendered(&mut self, scenario: &str) {
        if self.flow.scenario.as_deref() != Some(scenario) {
            return;
        }
        let Phase::Armed(seq) = self.flow.phase else {
            return;
        };
        bench::mark(scenario, seq, "render");
        self.flow.phase = Phase::Done;
        self.rearm = Some(seq + 1);
    }

    fn g8_poll(&mut self, since: Instant, window: &mut Window, cx: &mut Context<Self>) {
        let scenario = "replay_scroll";
        if self.wide(window) {
            let follow = self.console.read(cx).console.follow;
            bench::mark(scenario, 0, if follow { "follow_on" } else { "follow_off" });
            bench::emit(&bench::begin_line(bench::monotonic_ns(), scenario));
            self.flow.phase = Phase::Done;
            self.start_replay(true, cx);
        } else if since.elapsed() > CONSOLE_WAIT {
            bench::mark(scenario, 0, "console_hidden");
            self.flow.phase = Phase::Done;
        }
    }
}
