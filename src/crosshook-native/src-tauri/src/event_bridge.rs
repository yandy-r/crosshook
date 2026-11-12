//! Every Tauri event this shell emits. Payload construction and event names
//! come from `crosshook_app::ipc`; this module only subscribes, forwards each
//! [`WireEvent`] to the webview, and owns the relay tasks.
//!
//! - startup bridge: app bus -> `startup-job-finished`, the legacy scan
//!   events, `profiles-changed` and `launch-session-changed`
//! - launch relay: one task per session -> `launch-log`, `launch-diagnostic`,
//!   `launch-complete`, `injection-log`
//! - process relays: run-executable, update, prefix-dep and protonup progress
//! - bench replay: `launch-log` lines of the startup benchmark

mod launch;
mod process;

use crosshook_app::bootstrap::StartupHandle;
use crosshook_app::ipc::{startup, WireEvent};
use crosshook_app::EventReceiver;
use tauri::{AppHandle, Emitter as _};

pub(crate) use self::launch::{relay_launch, replay_launch_start_error};
pub(crate) use self::process::{
    relay_bench_replay, relay_prefix_deps, relay_protonup_progress, relay_run_executable,
    relay_update,
};

/// Delivers one wire event to every webview; a failure is logged, never fatal.
/// Returns whether the event was delivered.
fn emit(handle: &AppHandle, event: WireEvent) -> bool {
    let name = event.name;
    match handle.emit_str(name, event.payload) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(%error, event = name, "failed to emit event");
            false
        }
    }
}

/// Bridges the app bus to the webview. Holds only a startup handle and the
/// receiver, never an application clone.
pub(crate) fn spawn_startup_bridge(
    handle: AppHandle,
    startup: StartupHandle,
    mut rx: EventReceiver,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            for wire in startup::wire_events(event, || startup.jobs_snapshot()) {
                emit(&handle, wire);
            }
        }
    });
}
