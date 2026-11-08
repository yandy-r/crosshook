//! Startup-benchmark hooks (YAN-782 / #509). Inert unless `CROSSHOOK_BENCH=1`.
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
//!
//! READY marker: one `READY <CLOCK_MONOTONIC ns>` line on stderr, once per process.

mod env;
mod plan;
mod ready;
mod replay;

pub use env::bench_enabled;
pub use plan::ReplayPlan;
pub use ready::{signal_ready, ReadyAction};
pub use replay::replay_log;
