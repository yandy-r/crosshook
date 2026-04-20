use tokio::sync::broadcast;
use tokio::task;

use crate::launch::session::{TeardownReason, WatchdogOutcome};
use crate::platform::host_std_command;

use super::{
    descendants::collect_descendant_pids,
    host::{collect_host_descendant_pids, is_host_descendant_process_running, is_host_pid_alive},
    process::is_descendant_process_running,
    resolver::{is_target_pid_alive, resolve_watchdog_target},
    ShutdownTarget, GAMESCOPE_DESCENDANT_CLEANUP_DELAY, GAMESCOPE_NATURAL_EXIT_GRACE,
    GAMESCOPE_POLL_INTERVAL, GAMESCOPE_SIGTERM_WAIT, GAMESCOPE_STARTUP_POLL_ITERATIONS,
};

/// When gamescope wraps a Proton launch it becomes the direct child of
/// CrossHook but does **not** exit when the game inside it exits — lingering
/// clients (`mangoapp`, `winedevice.exe`, `gamescopereaper`) keep the
/// compositor alive indefinitely. This watchdog polls for the game executable
/// and, once it disappears, terminates gamescope so the normal
/// stream-log / finalize cleanup path can proceed.
///
/// The `cancel_rx` lets a parent launch (e.g. a game that owns a linked
/// trainer) short-circuit the poll loop and tear the tree down on demand.
/// Game launches receive a receiver that is never signalled; trainer launches
/// receive one wired to the launch-session registry so when the game session
/// ends the trainer watchdog cancels out of its poll loop immediately.
///
/// The `outcome` slot is written when the watchdog fires and is read by the
/// stream finalizer to stamp the launch's `diagnostic_json` with the reason.
pub async fn gamescope_watchdog(
    gamescope_pid: u32,
    exe_name: &str,
    outcome: WatchdogOutcome,
    host_pid_capture_path: Option<std::path::PathBuf>,
    mut cancel_rx: broadcast::Receiver<TeardownReason>,
) {
    let Some(observation_target) =
        resolve_watchdog_target(gamescope_pid, host_pid_capture_path.as_deref(), exe_name).await
    else {
        return;
    };

    // Startup poll: wait for the game process to appear inside the gamescope
    // subtree. Honor cancel here too — a trainer can be cancelled before its
    // child exe ever starts.
    let mut game_seen = false;
    for _ in 0..GAMESCOPE_STARTUP_POLL_ITERATIONS {
        if !is_target_pid_alive(observation_target) {
            return;
        }
        if descendant_process_running_for_target(observation_target, exe_name.to_string()).await {
            game_seen = true;
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(GAMESCOPE_POLL_INTERVAL) => {}
            reason = cancel_rx.recv() => {
                let reason = cancel_reason(reason);
                tracing::info!(
                    exe = %exe_name,
                    gamescope_pid = observation_target.pid,
                    teardown_reason = %reason,
                    "gamescope watchdog: cancel received during startup poll"
                );
                shutdown_gamescope_tree(observation_target, &outcome, reason).await;
                return;
            }
        }
    }
    if !game_seen {
        tracing::debug!(
            exe = %exe_name,
            "gamescope watchdog: game process never appeared, standing down"
        );
        return;
    }

    // Main poll loop: wait for the game process to exit inside gamescope, or
    // for a parent-driven cancel signal.
    loop {
        if !is_target_pid_alive(observation_target) {
            return;
        }
        if !descendant_process_running_for_target(observation_target, exe_name.to_string()).await {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(GAMESCOPE_POLL_INTERVAL) => {}
            reason = cancel_rx.recv() => {
                let reason = cancel_reason(reason);
                tracing::info!(
                    exe = %exe_name,
                    gamescope_pid = observation_target.pid,
                    teardown_reason = %reason,
                    "gamescope watchdog: cancel received during main poll"
                );
                shutdown_gamescope_tree(observation_target, &outcome, reason).await;
                return;
            }
        }
    }

    tracing::info!(
        exe = %exe_name,
        gamescope_pid = observation_target.pid,
        "gamescope watchdog: game exited, waiting for natural compositor shutdown"
    );

    tokio::time::sleep(GAMESCOPE_NATURAL_EXIT_GRACE).await;
    if !is_target_pid_alive(observation_target) {
        return;
    }

    shutdown_gamescope_tree(
        observation_target,
        &outcome,
        TeardownReason::WatchdogNaturalExit,
    )
    .await;
}

/// Kill the gamescope compositor and any lingering descendants for `target`.
/// Used both by the natural-exit path (game exe disappeared) and by the
/// cancel-initiated path (parent session broadcast a teardown reason).
///
/// Marks `outcome` so the stream finalizer can embed the reason in the
/// launch's `diagnostic_json`.
pub(crate) async fn shutdown_gamescope_tree(
    target: ShutdownTarget,
    outcome: &WatchdogOutcome,
    reason: TeardownReason,
) {
    let descendants = collect_descendant_pids_task(target).await;

    outcome.mark(reason);
    tracing::info!(
        gamescope_pid = target.pid,
        teardown_reason = %reason,
        "gamescope watchdog: sending SIGTERM"
    );
    signal_pid_task(target.pid, None).await;

    tokio::time::sleep(GAMESCOPE_SIGTERM_WAIT).await;
    if !is_target_pid_alive(target) {
        kill_remaining_descendants_task(descendants, target.host_namespace).await;
        return;
    }

    tracing::warn!(
        gamescope_pid = target.pid,
        teardown_reason = %reason,
        "gamescope watchdog: sending SIGKILL"
    );
    signal_pid_task(target.pid, Some("-KILL")).await;

    tokio::time::sleep(GAMESCOPE_DESCENDANT_CLEANUP_DELAY).await;
    kill_remaining_descendants_task(descendants, target.host_namespace).await;
}

/// Normalize a broadcast-channel receive result into a [`TeardownReason`].
/// On `RecvError::Closed` the parent registry is gone (unlikely in practice —
/// it's managed state for the app lifetime); on `Lagged` we already missed
/// earlier signals and still need to treat this as an instruction to tear
/// down. Both fall through as [`TeardownReason::LinkedSessionExit`].
fn cancel_reason(recv: Result<TeardownReason, broadcast::error::RecvError>) -> TeardownReason {
    match recv {
        Ok(reason) => reason,
        Err(broadcast::error::RecvError::Closed) => TeardownReason::LinkedSessionExit,
        Err(broadcast::error::RecvError::Lagged(_)) => TeardownReason::LinkedSessionExit,
    }
}

async fn descendant_process_running_for_target(target: ShutdownTarget, exe_name: String) -> bool {
    let exe_for_log = exe_name.clone();
    task::spawn_blocking(move || {
        if target.host_namespace {
            is_host_descendant_process_running(target.pid, &exe_name)
        } else {
            is_descendant_process_running(target.pid, &exe_name)
        }
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!(
            %error,
            gamescope_pid = target.pid,
            exe = %exe_for_log,
            host_namespace = target.host_namespace,
            "gamescope watchdog: descendant scan task failed"
        );
        false
    })
}

async fn collect_descendant_pids_task(target: ShutdownTarget) -> Vec<u32> {
    task::spawn_blocking(move || {
        if target.host_namespace {
            collect_host_descendant_pids(target.pid)
        } else {
            collect_descendant_pids(target.pid)
        }
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!(
            %error,
            gamescope_pid = target.pid,
            "gamescope watchdog: descendant collection task failed"
        );
        Vec::new()
    })
}

async fn kill_remaining_descendants_task(pids: Vec<u32>, host_namespace: bool) {
    if pids.is_empty() {
        return;
    }

    let descendant_count = pids.len();
    if let Err(error) =
        task::spawn_blocking(move || kill_remaining_descendants(&pids, host_namespace)).await
    {
        tracing::warn!(
            %error,
            descendant_count,
            "gamescope watchdog: descendant cleanup task failed"
        );
    }
}

async fn signal_pid_task(pid: u32, signal: Option<&'static str>) {
    let result = task::spawn_blocking(move || {
        let mut command = host_std_command("kill");
        if let Some(signal) = signal {
            command.arg(signal);
        }
        command.arg(pid.to_string()).status()
    })
    .await;

    match result {
        Ok(Ok(_status)) => {}
        Ok(Err(error)) => {
            tracing::warn!(%error, pid, ?signal, "gamescope watchdog: failed to signal process");
        }
        Err(error) => {
            tracing::warn!(
                %error,
                pid,
                ?signal,
                "gamescope watchdog: signal task join failed"
            );
        }
    }
}

fn kill_remaining_descendants(pids: &[u32], host_namespace: bool) {
    for &pid in pids {
        let alive = if host_namespace {
            is_host_pid_alive(pid)
        } else {
            super::process::is_pid_alive(pid)
        };
        if alive {
            tracing::info!(pid, "gamescope watchdog: killing orphaned descendant");
            // Accepted low-risk TOCTOU window: the pid could exit and be reused
            // between the liveness check and the signal delivery, but this cleanup
            // runs immediately after killing a same-UID gamescope tree on a
            // desktop system where reuse rates are low.
            let _ = host_std_command("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_reason_maps_ok() {
        assert_eq!(
            cancel_reason(Ok(TeardownReason::LinkedSessionExit)),
            TeardownReason::LinkedSessionExit,
        );
        assert_eq!(
            cancel_reason(Ok(TeardownReason::UserRequest)),
            TeardownReason::UserRequest,
        );
    }

    #[test]
    fn cancel_reason_maps_closed_to_linked_session_exit() {
        assert_eq!(
            cancel_reason(Err(broadcast::error::RecvError::Closed)),
            TeardownReason::LinkedSessionExit,
        );
    }

    #[test]
    fn cancel_reason_maps_lagged_to_linked_session_exit() {
        assert_eq!(
            cancel_reason(Err(broadcast::error::RecvError::Lagged(3))),
            TeardownReason::LinkedSessionExit,
        );
    }
}
