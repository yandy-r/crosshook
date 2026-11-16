//! Startup-benchmark hook. The env contract, READY marker and
//! replay pacing live in `crosshook_app::bench`; this command only forwards the
//! frontend-ready signal and emits the replayed lines.

pub use crosshook_app::bench::bench_enabled;

use crosshook_app::bench::{
    active_scenario, bench_begin as begin_marks, bench_clock as read_clock,
    bench_mark as write_mark, signal_ready, take_deferred_replay, BenchScenario, ReadyAction,
};
use tauri::AppHandle;

use super::IpcResult;
use crate::event_bridge::{relay_bench_replay, relay_bench_replay_after_go};

/// Frontend-ready signal for the startup benchmark. No-op unless `CROSSHOOK_BENCH=1`.
/// Returns the active benchmark scenario, `None` when bench mode is off or no scenario is set.
#[tauri::command]
pub async fn bench_ready(handle: AppHandle) -> IpcResult<Option<BenchScenario>> {
    match signal_ready()? {
        ReadyAction::None => {}
        ReadyAction::Exit => handle.exit(0),
        ReadyAction::Replay(plan) => relay_bench_replay(handle, plan),
    }
    Ok(active_scenario())
}

/// Start a benchmark marker sequence. No-op unless `CROSSHOOK_BENCH=1`. Starts a parked
/// replay (one waiting for the runner's `GO`) once the marks are recording.
#[tauri::command]
pub async fn bench_begin(handle: AppHandle) -> IpcResult<()> {
    begin_marks()?;
    if let Some(plan) = take_deferred_replay() {
        relay_bench_replay_after_go(handle, plan);
    }
    Ok(())
}

/// Record a scenario phase marker emitted by the frontend.
#[tauri::command]
pub async fn bench_mark(
    scenario: String,
    seq: u32,
    phase: String,
    t_perf_ms: f64,
) -> IpcResult<()> {
    write_mark(&scenario, seq, &phase, t_perf_ms)
}

/// Clock-correlation probe pairing a frontend `performance.now()` with the backend clock.
#[tauri::command]
pub async fn bench_clock(seq: u32, t_perf_ms: f64) -> IpcResult<u64> {
    read_clock(seq, t_perf_ms)
}
