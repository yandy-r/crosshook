//! Bench probe glue: what happens after READY, the G5 trial loop and the G8 console gate.

use std::time::{Duration, Instant};

use iced::Task;

use super::bench_flow::{BenchFlow, Phase};
use super::{App, Message, Route};
use crate::bench;
use crate::focus::Target;

const CONSOLE_WAIT: Duration = Duration::from_secs(30);

impl App {
    pub fn after_ready(&mut self) -> Task<Message> {
        if self.bench.exit_after_ready {
            return iced::exit();
        }
        if self.bench.is_g8() {
            self.flow.phase = Phase::WaitConsole(Instant::now());
            return self.g8_poll();
        }
        if !self.bench.is_g5() {
            self.flow.phase = Phase::Done;
            self.start_replay(false);
            return Task::none();
        }
        BenchFlow::clock_sync();
        self.flow.begin();
        let setup = match self.flow.scenario.as_deref() {
            Some("toggle_optimization") => {
                self.history.push(Route::Settings);
                Task::none()
            }
            _ => Task::none(),
        };
        Task::batch([setup, self.arm(0)])
    }

    /// Focuses the scenario target and sends `armed`.
    pub fn arm(&mut self, seq: u64) -> Task<Message> {
        let target = match self.flow.scenario.as_deref() {
            Some("search") => Target::Search,
            Some("toggle_favorite") => Target::Favorite(0),
            Some("toggle_optimization") => Target::Field(12),
            Some("dpad") => Target::Card(0),
            Some("route_switch") => {
                // Focus the non-active tab without activating it.
                Target::Nav(if self.route() == Route::Library { 1 } else { 0 })
            }
            _ => return Task::none(),
        };
        // `dpad` alternates Right/Left; arm from whichever card focus is on.
        let focus = if self.flow.scenario.as_deref() == Some("dpad")
            && matches!(self.focus.focused, Some(Target::Card(_)))
        {
            Task::none()
        } else {
            self.focus_to(target)
        };
        self.flow.armed(seq);
        focus
    }

    /// Called by every update that applies a scenario's change.
    pub fn bench_rendered(&mut self, scenario: &str) -> Task<Message> {
        if self.flow.scenario.as_deref() != Some(scenario) {
            return Task::none();
        }
        match self.flow.rendered() {
            Some(next) if next <= bench::MAX_SEQ => self.arm(next),
            _ => Task::none(),
        }
    }

    pub fn g8_poll(&mut self) -> Task<Message> {
        let Phase::WaitConsole(since) = self.flow.phase else {
            return Task::none();
        };
        let scenario = "replay_scroll";
        if self.wide() {
            let phase = if self.console.scroll.following() {
                "follow_on"
            } else {
                "follow_off"
            };
            bench::mark(scenario, 0, phase);
            self.flow.begin();
            self.flow.phase = Phase::Done;
            self.start_replay(true);
        } else if since.elapsed() > CONSOLE_WAIT {
            bench::mark(scenario, 0, "console_hidden");
            self.flow.phase = Phase::Done;
        }
        Task::none()
    }
}
