//! The one-shot READY marker: one `READY <CLOCK_MONOTONIC ns>` line on stderr
//! per process, after the frontend signals it is ready.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::deferred::park_deferred;
use super::env::{decide, BenchEnv, BenchMode};
use super::plan::{resolve_replay_plan, ReplayPlan};
use super::scenario::{parse_scenario, Scenario};
use super::util::{claim, monotonic_ns, write_line};

static READY_FIRED: AtomicBool = AtomicBool::new(false);

/// What the shell does after the READY marker was written.
#[derive(Debug)]
pub enum ReadyAction {
    /// Benchmark mode is off or READY already fired: nothing to do.
    None,
    /// Exit the process right away (`CROSSHOOK_BENCH_EXIT_AFTER_READY=1`).
    Exit,
    /// Replay the log through [`replay_log`](super::replay_log). Never returned for a scenario
    /// that defers the replay: READY parks that plan for
    /// [`take_deferred_replay`](super::take_deferred_replay) and returns [`ReadyAction::None`].
    Replay(ReplayPlan),
}

#[derive(Debug)]
struct Started {
    ns: u64,
    exit_after_ready: bool,
    replay: Option<ReplayPlan>,
    /// The active scenario starts the replay later (`bench_begin` plus the runner's go line).
    defer_replay: bool,
}

/// Whether the configured scenario defers the replay. An unknown scenario does not: it is
/// reported by [`active_scenario`](super::active_scenario), and READY behaves as without one.
fn defers_replay(env: &BenchEnv) -> bool {
    parse_scenario(env)
        .ok()
        .flatten()
        .is_some_and(Scenario::defers_replay)
}

/// Validate everything, then claim the one-shot. Any error leaves the flag unclaimed.
/// `Ok(None)`: disabled or already fired.
fn begin(flag: &AtomicBool, env: &BenchEnv) -> Result<Option<Started>, String> {
    let BenchMode::Ready {
        exit_after_ready,
        replay,
    } = decide(
        env.bench.as_deref(),
        env.exit_after_ready.as_deref(),
        env.log.as_deref(),
    )
    else {
        return Ok(None);
    };
    if flag.load(Ordering::Acquire) {
        return Ok(None);
    }
    let replay = match env.log.as_deref().filter(|_| replay) {
        Some(log) => Some(resolve_replay_plan(
            env.root.as_deref(),
            log,
            env.rate.as_deref(),
            std::env::var_os("HOME").as_deref().map(Path::new),
        )?),
        None => None,
    };
    let ns = monotonic_ns()?;
    if !claim(flag) {
        return Ok(None);
    }
    Ok(Some(Started {
        ns,
        exit_after_ready,
        replay,
        defer_replay: defers_replay(env),
    }))
}

fn write_ready(ns: u64) -> Result<(), String> {
    write_line(&format!("READY {ns}")).map_err(|e| format!("failed to write READY marker: {e}"))
}

/// Hand the replay plan to the shell, or park it when the scenario defers the replay.
fn route_replay(
    replay: Option<ReplayPlan>,
    defer: bool,
    park: impl FnOnce(ReplayPlan),
) -> ReadyAction {
    match replay {
        Some(plan) if defer => {
            park(plan);
            ReadyAction::None
        }
        Some(plan) => ReadyAction::Replay(plan),
        None => ReadyAction::None,
    }
}

/// Frontend-ready signal for the startup benchmark. A no-op unless
/// `CROSSHOOK_BENCH=1`; otherwise validates the environment, writes the READY
/// marker once and says what the shell does next.
///
/// # Errors
/// A configuration error (also printed as `BENCH_ERROR:` on stderr so the
/// runner sees why READY never came; messages carry env var names and OS error
/// text only, never file contents) or a failed marker write.
pub fn signal_ready() -> Result<ReadyAction, String> {
    let started = begin(&READY_FIRED, &BenchEnv::from_process()).inspect_err(|error| {
        eprintln!("BENCH_ERROR: {error}");
    })?;
    let Some(started) = started else {
        return Ok(ReadyAction::None);
    };
    write_ready(started.ns)?;
    Ok(if started.exit_after_ready {
        ReadyAction::Exit
    } else {
        route_replay(started.replay, started.defer_replay, park_deferred)
    })
}

#[cfg(test)]
mod tests;
