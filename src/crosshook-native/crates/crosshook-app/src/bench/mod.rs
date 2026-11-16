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
//!   `toggle_optimization`, `dpad`, `route_switch`, `health_table` (the route switch into the
//!   health table); G4 scenario: `health_scroll` (the health page scroll); G8 scenario:
//!   `replay_scroll`, which defers the replay (anything else: `BENCH_ERROR:`, no scenario).
//!   Neither health scenario defers the replay
//!
//! READY marker: one `READY <CLOCK_MONOTONIC ns>` line on stderr, once per process.
//!
//! Scenario and markers (all inert unless `CROSSHOOK_BENCH=1`, every line one `writeln!`):
//! - [`active_scenario`] reads the scenario. `signal_ready` is unchanged (it still
//!   returns [`ReadyAction`]), so the shell asks for the scenario separately.
//! - [`bench_begin`]: `BENCH_BEGIN <ns> <scenario>`, once per process.
//! - [`bench_mark`]: `BENCH_MARK <ns> <scenario> <seq> <phase> <t_perf_us>`; the scenario
//!   must be the active one, `phase` is `armed`/`render`/`focus_lost`/`console_hidden`/`follow_on`/`follow_off`/
//!   `health_ready`/`health_rows_too_few`, `seq <= 1_000_000`, `t_perf_ms` finite within
//!   `0..=86_400_000` (emitted as integer microseconds). The health probes send
//!   `health_ready` (the live health table reached its minimum row count) or
//!   `health_rows_too_few` (it did not) at seq 0.
//! - [`bench_clock`]: `BENCH_CLOCK <ns> <seq> <t_perf_us>`, returns `ns` (`0` when off).
//! - Deferred replay (`CROSSHOOK_BENCH_SCENARIO=replay_scroll`, with `_LOG`): READY does not
//!   start the replay; it parks the plan. After a successful [`bench_begin`] the shell calls
//!   [`take_deferred_replay`] (once) and spawns [`replay_after_go`]. That waits up to
//!   `GO_TIMEOUT_S` (120 s) for exactly `GO\n` on stdin, read on a plain thread with `poll(2)`
//!   and only in this case, then replays at the configured rate, writing
//!   `BENCH_REPLAY <ns> start <total_lines>` before the first line and
//!   `BENCH_REPLAY <ns> done <sent>` after the last (integers only, each one write). A missing
//!   or wrong go line writes one `BENCH_ERROR: GO_SIGNAL_FAILED: <reason>` and replays nothing.
//!   Without the scenario (G4 `console`) the replay still starts at READY and writes no
//!   `BENCH_REPLAY` lines.
//! - Marks and clock samples are each capped at 100_000 per process: one
//!   `BENCH_ERROR: marker cap reached`, then further samples are dropped with `Ok`.

mod deferred;
mod env;
mod go;
mod mark;
mod plan;
mod ready;
mod replay;
mod scenario;
mod util;

pub use deferred::take_deferred_replay;
pub use env::bench_enabled;
pub use mark::{bench_begin, bench_clock, bench_mark};
pub use plan::ReplayPlan;
pub use ready::{signal_ready, ReadyAction};
pub use replay::{replay_after_go, replay_log};
pub use scenario::{active_scenario, BenchScenario};
