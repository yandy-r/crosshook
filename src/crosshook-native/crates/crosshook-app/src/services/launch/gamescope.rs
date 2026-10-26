//! Gamescope predicate, session plan, cancel-consumer choice and spawn.
//!
//! One pure predicate replaces the four copies the Tauri layer carried. Host
//! facts arrive through [`LaunchEnv`], so the truth table is testable without
//! environment guards. `should_skip_gamescope` in core stays untouched: it
//! decides builder arguments, this module decides watchdog gating.
//!
//! Nothing here signals a process directly; teardown goes through the core
//! watchdog only.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crosshook_core::launch::script_runner::gamescope_pid_capture_path;
use crosshook_core::launch::{
    drain_cancel_into_outcome, gamescope_watchdog, LaunchRequest, SessionId, SessionKind,
    TeardownReason, WatchdogOutcome, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::platform::portals::background::background_supported;
use crosshook_core::profile::{resolve_trainer_launch_method, GamescopeConfig};
use tokio::sync::broadcast;

use super::types::LaunchEnv;
use crate::services::BackgroundGrantHolder;

/// How long the watchdog start waits for the Background-portal grant.
/// Diagnostic only: the wait never gates the launch result.
const GRANT_WAIT: Duration = Duration::from_millis(500);

/// The method the session's process actually executes with.
///
/// A game runs with its resolved method. A trainer runs with
/// `resolve_trainer_launch_method`: a Steam trainer under Flatpak runs through
/// the direct Proton builder.
pub fn execution_method(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> &'static str {
    let resolved = match req.resolved_method() {
        METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
        METHOD_PROTON_RUN => METHOD_PROTON_RUN,
        _ => METHOD_NATIVE,
    };
    match kind {
        SessionKind::Game => resolved,
        SessionKind::Trainer => resolve_trainer_launch_method(resolved, env.is_flatpak),
    }
}

/// Whether CrossHook wraps the session's process in gamescope.
///
/// True only for an execution method of `proton_run` with an enabled gamescope
/// config that is allowed to run here: nested use is allowed, or no gamescope
/// session surrounds the process. The config is the game's for a game session
/// and the resolved trainer config for a trainer session.
pub fn gamescope_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool {
    if execution_method(req, kind, env) != METHOD_PROTON_RUN {
        return false;
    }
    let config = session_gamescope(req, kind);
    config.enabled && (config.allow_nested || !env.inside_gamescope_session)
}

/// Whether the watchdog is handed a gamescope PID-capture path: the predicate
/// holds and the process runs inside the Flatpak sandbox.
///
/// Kept divergence (decision D8): for a Flatpak trainer the path reaches the
/// watchdog, but the trainer builder never writes the capture file, so the
/// watchdog falls back to the host ancestor search after its poll window.
pub fn pid_capture_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool {
    gamescope_required(req, kind, env) && env.is_flatpak
}

/// The gamescope facts of one session, decided once before spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GamescopePlan {
    /// [`gamescope_required`] for the session.
    pub required: bool,
    /// `<log>.gamescope.pid` when [`pid_capture_required`] holds.
    pub pid_capture: Option<PathBuf>,
}

/// Evaluates the predicates once for a session logging to `log_path`.
pub(super) fn gamescope_plan(
    req: &LaunchRequest,
    kind: SessionKind,
    env: LaunchEnv,
    log_path: &Path,
) -> GamescopePlan {
    GamescopePlan {
        required: gamescope_required(req, kind, env),
        pid_capture: pid_capture_required(req, kind, env)
            .then(|| gamescope_pid_capture_path(log_path)),
    }
}

/// What consumes a session's cancel channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CancelConsumer {
    /// Gamescope wraps the child: the watchdog tears the compositor tree down.
    Watchdog {
        /// Direct child (gamescope) PID.
        pid: u32,
        /// Basename of the executable the watchdog waits on; may be empty.
        exe_name: String,
        /// Host PID-capture file, when the plan has one.
        capture: Option<PathBuf>,
    },
    /// Nothing to tear down: only record the cancel reason.
    Drain,
}

/// Picks the cancel consumer: [`CancelConsumer::Watchdog`] iff gamescope is
/// required and the child has a PID, otherwise [`CancelConsumer::Drain`].
///
/// The executable is the basename of `game_path` for a game and of
/// `trainer_host_path` for a trainer, both taken unmodified from the request.
pub(super) fn choose_cancel_consumer(
    plan: &GamescopePlan,
    req: &LaunchRequest,
    kind: SessionKind,
    child_pid: Option<u32>,
) -> CancelConsumer {
    if !plan.required {
        return CancelConsumer::Drain;
    }
    let Some(pid) = child_pid else {
        // `Child::id` is `None` only after `wait()`; drain so a cascade still
        // attributes its reason.
        tracing::warn!("gamescope-active launch missing child PID; falling back to cancel-drain");
        return CancelConsumer::Drain;
    };
    let source = match kind {
        SessionKind::Game => req.game_path.as_str(),
        SessionKind::Trainer => req.trainer_host_path.as_str(),
    };
    CancelConsumer::Watchdog {
        pid,
        exe_name: exe_basename(source),
        capture: plan.pid_capture.clone(),
    }
}

/// Starts the task that owns `cancel_rx` for the rest of the session.
///
/// Every session gets exactly one consumer, so a cascade or a user reset
/// always has a live receiver. Must be called inside a Tokio runtime.
///
/// A `Watchdog` with an empty executable name keeps today's behavior
/// (decision D8, invariant I-32): it logs, returns and drops `cancel_rx`, so
/// cascades into that session are lost.
pub(super) fn spawn_cancel_consumer(
    consumer: CancelConsumer,
    session_id: SessionId,
    outcome: WatchdogOutcome,
    cancel_rx: broadcast::Receiver<TeardownReason>,
    background: Option<Arc<BackgroundGrantHolder>>,
) {
    match consumer {
        CancelConsumer::Drain => {
            tokio::spawn(drain_cancel_into_outcome(session_id, outcome, cancel_rx));
        }
        CancelConsumer::Watchdog {
            pid,
            exe_name,
            capture,
        } => {
            if exe_name.is_empty() {
                tracing::warn!(
                    session_id = %session_id,
                    gamescope_pid = pid,
                    "gamescope watchdog disabled: launch path did not yield an executable basename"
                );
                return;
            }
            tokio::spawn(async move {
                log_background_grant(pid, background.as_deref()).await;
                gamescope_watchdog(pid, &exe_name, outcome, capture, cancel_rx).await;
            });
        }
    }
}

/// Waits up to [`GRANT_WAIT`] for the Background-portal grant and logs its
/// state. Only runs where the portal exists (Flatpak) and a holder was given.
async fn log_background_grant(gamescope_pid: u32, holder: Option<&BackgroundGrantHolder>) {
    if !background_supported() {
        return;
    }
    let Some(holder) = holder else {
        return;
    };
    let state = holder.wait_for_initialization(GRANT_WAIT).await;
    tracing::info!(
        gamescope_pid,
        protection_state = ?state,
        active_grant = holder.has_active_grant(),
        initialized = holder.is_initialized(),
        "background portal grant state at watchdog spawn"
    );
}

/// The gamescope config governing a session of `kind`.
fn session_gamescope(req: &LaunchRequest, kind: SessionKind) -> GamescopeConfig {
    match kind {
        SessionKind::Game => req.gamescope.clone(),
        SessionKind::Trainer => req.resolved_trainer_gamescope(),
    }
}

/// `Path::file_name` of `path`, or `""` when it has none or is not UTF-8.
fn exe_basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}
