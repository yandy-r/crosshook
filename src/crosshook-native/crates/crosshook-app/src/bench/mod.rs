//! Startup-benchmark hooks. Inert unless `CROSSHOOK_BENCH=1`.
//!
//! Toolkit-independent: the env decision, replay-plan validation, the
//! one-shot monotonic READY marker and the paced log reader live here so the
//! native UI benchmark harness can reuse the same contract; a shell only
//! forwards the frontend-ready signal and emits the replayed lines.
//!
//! Env contract:
//! - `CROSSHOOK_BENCH=1`                  enable hooks (anything else: no-op)
//! - `CROSSHOOK_BENCH_EXIT_AFTER_READY=1` exit(0) right after the READY marker
//! - `CROSSHOOK_BENCH_ROOT`               absolute fixture root dir (required with `_LOG`)
//! - `CROSSHOOK_BENCH_LOG`                absolute log file under the root; replayed as `launch-log`
//! - `CROSSHOOK_BENCH_LOG_RATE`           lines/s, 1..=5000 (default 5000)
//! - `CROSSHOOK_BENCH_SCENARIO`           G5 scenario: `search`, `toggle_favorite`,
//!   `toggle_optimization`, `dpad`, `route_switch` (anything else: `BENCH_ERROR:`, no scenario)
//!
//! READY marker: one `READY <CLOCK_MONOTONIC ns>` line on stderr, once per process.
//!
//! Scenario and markers (all inert unless `CROSSHOOK_BENCH=1`, every line one `writeln!`):
//! - [`active_scenario`] reads the scenario. `signal_ready` is unchanged (it still
//!   returns [`ReadyAction`]), so the shell asks for the scenario separately.
//! - [`bench_begin`]: `BENCH_BEGIN <ns> <scenario>`, once per process.
//! - [`bench_mark`]: `BENCH_MARK <ns> <scenario> <seq> <phase> <t_perf_us>`; the scenario
//!   must be the active one, `phase` is `armed`/`render`/`focus_lost`, `seq <= 1_000_000`,
//!   `t_perf_ms` finite within `0..=86_400_000` (emitted as integer microseconds).
//! - [`bench_clock`]: `BENCH_CLOCK <ns> <seq> <t_perf_us>`, returns `ns` (`0` when off).
//! - Marks and clock samples are each capped at 100_000 per process: one
//!   `BENCH_ERROR: marker cap reached`, then further samples are dropped with `Ok`.

mod env;
mod mark;
mod plan;
mod ready;
mod replay;
mod scenario;
mod util;

pub use env::bench_enabled;
pub use mark::{bench_begin, bench_clock, bench_mark};
pub use plan::ReplayPlan;
pub use ready::{signal_ready, ReadyAction};
pub use replay::replay_log;
pub use scenario::{active_scenario, BenchScenario};
