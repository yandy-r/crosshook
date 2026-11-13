//! Steam game exit watch.
//!
//! A `steam_applaunch` game session's child is `steam-launch-helper.sh`,
//! which starts the game through Steam and exits about 30 s later while the
//! game keeps running. For those sessions this watcher replaces the plain
//! cancel drain and owns deregistration:
//!
//! - Until the helper exits it records any cancel reason, like the drain.
//!   A reset during the helper's startup only records the reason: the
//!   session ends when the helper exits (at the handoff, or through the
//!   finalizer when the helper failed).
//! - When the helper exits successfully (code 0, no signal), the finalizer
//!   hands its deregister step off (a oneshot carrying the appear window)
//!   instead of deregistering. The watcher waits up to that window
//!   ([`APPEAR_TIMEOUT`]) for the game executable to be visible, then polls
//!   every [`POLL_INTERVAL`] until it has been missing for
//!   [`EXIT_MISS_THRESHOLD`] consecutive probes, and deregisters with no exit
//!   status. A game that never appears is deregistered at the timeout. A
//!   probe that cannot tell (the host `ps` query failed) neither counts as a
//!   miss nor resets the count.
//! - A helper killed by a signal may have been signalled before or after its
//!   `steam -applaunch` line, so it hands off too, but with the short
//!   [`SIGNALLED_APPEAR_TIMEOUT`] window.
//! - A helper that exited with a non-zero code, or could not be waited on
//!   (no status), never started the game: the finalizer deregisters with the
//!   real status and drops the handoff sender, which ends the watcher
//!   without probing.
//! - After the handoff, a cancel (the user reset) or a closed cancel channel
//!   deregisters at once.
//!
//! The watcher never signals or kills anything: it only probes by executable
//! name (same name-based match as the old UI poll, so a same-named process
//! keeps the session alive). If the finalizer never hands off (the session
//! task was dropped or panicked), its `Drop` deregisters as before and the
//! watcher exits when the handoff sender is dropped.

use std::sync::Arc;
use std::time::Duration;

use crosshook_core::launch::{
    LaunchRequest, LaunchSessionRegistry, SessionId, SessionKind, TeardownReason, WatchdogOutcome,
    METHOD_STEAM_APPLAUNCH,
};
use tokio::sync::{broadcast, oneshot};
use tokio::time::Instant;

use super::run_blocking;
use crate::services::BoxFuture;

/// How long the game may take to become visible after the helper exited.
///
/// Deliberately generous: Steam can run shader pre-caching, a game update or
/// a first-run installer between the helper's exit and the game process
/// appearing, and ending the session then would show a running game as
/// stopped. The cost of a long value is only how long a game that never
/// appears (Steam refused or the user closed a dialog) lingers as running;
/// a reset still ends it immediately.
pub(super) const APPEAR_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Appear window after the helper was killed by a signal.
///
/// The helper starts the game in the background and does not forward
/// signals, so a signal after its `steam -applaunch` line leaves the game
/// starting while a signal before it starts nothing, and the exit status
/// cannot tell the two apart. A game Steam was told to start is normally
/// visible within this window; a session whose launch never happened
/// lingers only this long instead of [`APPEAR_TIMEOUT`].
pub(super) const SIGNALLED_APPEAR_TIMEOUT: Duration = Duration::from_secs(60);

/// Probe cadence, matching the gamescope watchdog's poll interval.
pub(super) const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Consecutive negative probes that end a visible game's session; any
/// positive probe resets the count, and an unknown probe does neither.
///
/// One miss is not proof the game exited: some games briefly re-exec
/// themselves. The registry session cannot come back after a false
/// negative, so the watch waits about `3 * POLL_INTERVAL` (6 s) of absence
/// before ending it.
pub(super) const EXIT_MISS_THRESHOLD: u32 = 3;

/// Whether a process with the given executable name is running: `Some` when
/// known, `None` when the probe could not tell. Production runs
/// [`crosshook_core::launch::probe_process_running`] on the blocking pool
/// (it may shell out to host `ps` under Flatpak); tests return ready
/// futures so paused Tokio time stays deterministic.
pub(super) type ProcessProbe =
    Arc<dyn Fn(String) -> BoxFuture<'static, Option<bool>> + Send + Sync>;

/// The production probe. A failed host `ps` query or a failed blocking task
/// is unknown, not "not visible".
pub(super) fn default_probe() -> ProcessProbe {
    Arc::new(|exe: String| {
        Box::pin(async move {
            match run_blocking(move || Ok(crosshook_core::launch::probe_process_running(&exe)))
                .await
            {
                Ok(visible) => visible,
                Err(error) => {
                    tracing::warn!(%error, "steam game probe failed");
                    None
                }
            }
        })
    })
}

/// Executable name to watch when the session is a Steam game launch, or
/// `None` when the plain cancel consumer applies (other methods, trainers,
/// trainer-only requests, or no usable executable name).
pub(super) fn steam_game_exe(
    request: &LaunchRequest,
    kind: SessionKind,
    execution_method: &str,
) -> Option<String> {
    if kind != SessionKind::Game
        || execution_method != METHOD_STEAM_APPLAUNCH
        || request.launch_trainer_only
    {
        return None;
    }
    let exe = request.game_executable_name();
    (!exe.is_empty()).then_some(exe)
}

/// Everything one watch needs.
pub(super) struct SteamWatch {
    pub session_id: SessionId,
    pub exe_name: String,
    pub registry: Arc<LaunchSessionRegistry>,
    pub outcome: WatchdogOutcome,
    pub probe: ProcessProbe,
}

/// Runs the watch to completion. Spawned as the session's cancel consumer;
/// `handoff` resolves with the appear window when the finalizer passes its
/// deregister step over.
pub(super) async fn watch_steam_game(
    watch: SteamWatch,
    mut cancel_rx: broadcast::Receiver<TeardownReason>,
    mut handoff: oneshot::Receiver<Duration>,
) {
    let mut cancelled = false;
    // Phase 1: the helper is still running.
    let appear_timeout = loop {
        tokio::select! {
            handed = &mut handoff => match handed {
                Ok(appear_timeout) => break appear_timeout,
                // The finalizer deregistered itself (drop path).
                Err(_) => return,
            },
            received = cancel_rx.recv(), if !cancelled => match received {
                Ok(reason) => {
                    watch.outcome.record_reason(reason);
                    cancelled = true;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    if let Some(reason) = latest_reason(&mut cancel_rx) {
                        watch.outcome.record_reason(reason);
                    }
                    cancelled = true;
                }
                // The entry is gone already; the finalizer's handoff will
                // fail and it deregisters (a no-op) itself.
                Err(broadcast::error::RecvError::Closed) => return,
            },
        }
    };

    // Phase 2: the helper exited and the watcher owns deregistration.
    if !cancelled {
        tokio::select! {
            () = wait_for_game_exit(&watch, appear_timeout) => {}
            received = cancel_rx.recv() => {
                if let Ok(reason) = received {
                    watch.outcome.record_reason(reason);
                }
            }
        }
    }
    let removed = watch
        .registry
        .deregister_with_exit(watch.session_id, None, None);
    tracing::info!(
        session_id = %watch.session_id,
        removed,
        cancelled,
        "steam game watch finished"
    );
}

/// Waits up to `appear_timeout` for the game to appear, then for it to stay
/// gone for [`EXIT_MISS_THRESHOLD`] consecutive known misses. Unknown probes
/// before the game appeared only let the appear window run on.
async fn wait_for_game_exit(watch: &SteamWatch, appear_timeout: Duration) {
    let deadline = Instant::now() + appear_timeout;
    while probe(watch).await != Some(true) {
        if Instant::now() >= deadline {
            tracing::info!(
                session_id = %watch.session_id,
                "steam game never became visible; ending the session"
            );
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let mut misses = 0;
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        match probe(watch).await {
            Some(true) => misses = 0,
            Some(false) => {
                misses += 1;
                if misses >= EXIT_MISS_THRESHOLD {
                    return;
                }
            }
            None => tracing::debug!(
                session_id = %watch.session_id,
                misses,
                "steam game probe could not tell; keeping the miss count"
            ),
        }
    }
}

async fn probe(watch: &SteamWatch) -> Option<bool> {
    (watch.probe)(watch.exe_name.clone()).await
}

/// The most recent reason still buffered after a lag.
fn latest_reason(cancel_rx: &mut broadcast::Receiver<TeardownReason>) -> Option<TeardownReason> {
    let mut latest = None;
    while let Ok(reason) = cancel_rx.try_recv() {
        latest = Some(reason);
    }
    latest
}
