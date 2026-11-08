//! Relays of supervised process jobs and the bench replay: each spawns one
//! task that emits the events its `crosshook_app::ipc` mapping produces.

use crosshook_app::bench::{self, ReplayPlan};
use crosshook_app::ipc::process::{prefix_deps, run_executable, update};
use crosshook_app::ipc::{protonup, WireEvent};
use crosshook_app::services::prefix_deps::DepInstallCompletion;
use crosshook_app::services::protonup::next_progress;
use crosshook_app::services::{ProcessRun, RunCompletion, RunEvent, UpdateCompletion};
use crosshook_core::protonup::progress::ProtonInstallProgress;
use tauri::AppHandle;
use tokio::sync::broadcast;

use super::emit;

/// Emits `map(run_id, event)` for every event of `run`, in order. The
/// completion arrives only after the service finished the job.
fn relay_run<C: Send + 'static>(
    handle: AppHandle,
    mut run: ProcessRun<C>,
    map: impl Fn(&str, RunEvent<C>) -> Vec<WireEvent> + Send + 'static,
) {
    drop(tauri::async_runtime::spawn(async move {
        let run_id = run.run_id().to_string();
        while let Some(event) = run.next().await {
            for wire in map(&run_id, event) {
                emit(&handle, wire);
            }
        }
    }));
}

/// Relays a run-executable run: `run-executable-log`, then
/// `run-executable-complete`, both tagged with the run id.
pub(crate) fn relay_run_executable(handle: AppHandle, run: ProcessRun<RunCompletion>) {
    relay_run(handle, run, run_executable::wire_events);
}

/// Relays an update run: `update-log` lines, then `update-complete`.
pub(crate) fn relay_update(handle: AppHandle, run: ProcessRun<UpdateCompletion>) {
    relay_run(handle, run, |_, event| update::wire_events(event));
}

/// Relays a dependency install with the raw request `profile_name` and
/// `prefix_path` (listeners match on the exact strings they sent).
pub(crate) fn relay_prefix_deps(
    handle: AppHandle,
    profile_name: String,
    prefix_path: String,
    run: ProcessRun<DepInstallCompletion>,
) {
    relay_run(handle, run, move |_, event| {
        prefix_deps::wire_events(&profile_name, &prefix_path, event)
    });
}

/// Relays a supervised Proton install's progress. `next_progress` skips
/// lagged snapshots and ends only when the sender closes.
pub(crate) fn relay_protonup_progress(
    handle: AppHandle,
    mut progress: broadcast::Receiver<ProtonInstallProgress>,
) {
    drop(tauri::async_runtime::spawn(async move {
        while let Some(progress) = next_progress(&mut progress).await {
            if let Some(wire) = protonup::progress_event(&progress) {
                emit(&handle, wire);
            }
        }
    }));
}

/// Replays the benchmark log as `launch-log` events.
pub(crate) fn relay_bench_replay(handle: AppHandle, plan: ReplayPlan) {
    drop(tauri::async_runtime::spawn(bench::replay_log(
        plan,
        move |wire| emit(&handle, wire),
    )));
}
