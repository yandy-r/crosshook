//! Startup-benchmark hook (YAN-782 / #509). The env contract, READY marker and
//! replay pacing live in `crosshook_app::bench`; this command only forwards the
//! frontend-ready signal and emits the replayed lines.

pub use crosshook_app::bench::bench_enabled;

use crosshook_app::bench::{signal_ready, ReadyAction};
use tauri::AppHandle;

use super::IpcResult;
use crate::event_bridge::relay_bench_replay;

/// Frontend-ready signal for the startup benchmark. No-op unless `CROSSHOOK_BENCH=1`.
#[tauri::command]
pub async fn bench_ready(handle: AppHandle) -> IpcResult<()> {
    match signal_ready()? {
        ReadyAction::None => {}
        ReadyAction::Exit => handle.exit(0),
        ReadyAction::Replay(plan) => relay_bench_replay(handle, plan),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = bench_ready;
    }
}
