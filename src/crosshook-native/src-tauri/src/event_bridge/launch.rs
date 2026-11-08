//! Per-session launch relay: drains one session's [`LaunchEvents`] in order
//! and emits the mapped `launch-*` events.

use crosshook_app::ipc::launch::{wire_events, LaunchLogRelayState};
use crosshook_app::{LaunchEvent, LaunchEvents};
use tauri::AppHandle;

use super::emit;

/// One relay task per session, draining the stream in order.
pub(crate) fn relay_launch(handle: AppHandle, mut events: LaunchEvents) {
    tauri::async_runtime::spawn(async move {
        let mut state = LaunchLogRelayState::default();
        while let Some(event) = events.recv().await {
            for wire in wire_events(&mut state, event) {
                emit(&handle, wire);
            }
        }
    });
}

/// Relays events published before a launch failed to start (pre-spawn
/// injection rows), so they are not lost with the `Err` return.
pub(crate) fn replay_launch_start_error(handle: &AppHandle, events: Vec<LaunchEvent>) {
    let mut state = LaunchLogRelayState::default();
    for event in events {
        for wire in wire_events(&mut state, event) {
            emit(handle, wire);
        }
    }
}
