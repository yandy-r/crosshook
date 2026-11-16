//! The G5/G8 scenario allowlist and the `CROSSHOOK_BENCH_SCENARIO` decision.

use std::sync::atomic::AtomicBool;

use serde::Serialize;

use super::env::{BenchEnv, ENV_SCENARIO};
use super::util::claim;

static SCENARIO_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);

/// Compiled-in scenario names, in the order the runner drives them.
const ALL: [Scenario; 6] = [
    Scenario::Search,
    Scenario::ToggleFavorite,
    Scenario::ToggleOptimization,
    Scenario::Dpad,
    Scenario::RouteSwitch,
    Scenario::ReplayScroll,
];

/// The scenario the runner selected for this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BenchScenario {
    /// Allowlisted scenario name (the token the runner expects on stderr).
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scenario {
    Search,
    ToggleFavorite,
    ToggleOptimization,
    Dpad,
    RouteSwitch,
    ReplayScroll,
}

impl Scenario {
    /// The token written to stderr; always a compiled-in literal, never input.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::ToggleFavorite => "toggle_favorite",
            Self::ToggleOptimization => "toggle_optimization",
            Self::Dpad => "dpad",
            Self::RouteSwitch => "route_switch",
            Self::ReplayScroll => "replay_scroll",
        }
    }

    /// True when READY must not start the log replay: the runner starts it with a `GO`
    /// line on stdin once the frontend probe has called `bench_begin`.
    pub(super) fn defers_replay(self) -> bool {
        matches!(self, Self::ReplayScroll)
    }

    /// Exact match against the allowlist; no trimming, no case folding.
    pub(super) fn parse(raw: &str) -> Option<Self> {
        ALL.into_iter().find(|scenario| scenario.name() == raw)
    }

    pub(super) fn public(self) -> BenchScenario {
        BenchScenario { name: self.name() }
    }
}

/// Pure decision. `Ok(None)`: bench mode off or no scenario requested.
/// The gate is checked first, so a disabled process never parses the value.
pub(super) fn parse_scenario(env: &BenchEnv) -> Result<Option<Scenario>, String> {
    if !env.enabled() {
        return Ok(None);
    }
    let Some(raw) = env.scenario.as_deref() else {
        return Ok(None);
    };
    Scenario::parse(raw)
        .map(Some)
        .ok_or_else(|| format!("{ENV_SCENARIO} is not a known scenario, got {raw:?}"))
}

/// Report a scenario error on stderr at most once per process.
fn report_once(flag: &AtomicBool, result: Result<Option<Scenario>, String>) -> Option<Scenario> {
    result
        .inspect_err(|error| {
            if claim(flag) {
                eprintln!("BENCH_ERROR: {error}");
            }
        })
        .ok()
        .flatten()
}

/// The scenario selected through `CROSSHOOK_BENCH_SCENARIO`. `None` unless
/// `CROSSHOOK_BENCH=1` and the value is on the allowlist; an invalid value also
/// prints one `BENCH_ERROR:` line (value `{:?}`-escaped).
pub fn active_scenario() -> Option<BenchScenario> {
    report_once(
        &SCENARIO_ERROR_REPORTED,
        parse_scenario(&BenchEnv::from_process()),
    )
    .map(Scenario::public)
}

#[cfg(test)]
mod tests;
