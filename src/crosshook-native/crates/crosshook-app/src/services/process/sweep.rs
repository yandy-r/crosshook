//! The prefix sweep every prefix-touching process job runs after its child
//! was cancelled or stopped.
//!
//! The job's process-group `SIGTERM`/`SIGKILL` cannot reach processes that
//! called `setsid()`: wineserver daemonizes into its own session, Wine
//! starts GUI processes (the executable, `explorer.exe`, `services.exe`, …)
//! double-forked into new sessions, umu-run spawns its child with
//! `start_new_session=True`, and pressure-vessel runs bwrap with
//! `--new-session`. They still carry the prefix in `WINEPREFIX` /
//! `STEAM_COMPAT_DATA_PATH`, so the sweep finds and `SIGKILL`s them by that.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::child::ChildError;
use crate::jobs::{sweep_prefix_until_clear, ChildExit, JobContext, PrefixSweep};

/// How long the sweep keeps killing and rescanning until no process uses the
/// prefix. Survivors of `SIGKILL` past this are reported, not waited for.
pub(crate) const SWEEP_GRACE: Duration = Duration::from_secs(1);

/// Sweeps `prefix` on the blocking pool (see [`sweep_prefix_until_clear`]).
///
/// A sweep task that could not run is reported as
/// [`PrefixSweep::Incomplete`] with no counted survivors, so callers fail
/// closed (a throwaway prefix is then kept).
pub(crate) async fn sweep_prefix(prefix: PathBuf) -> PrefixSweep {
    sweep_prefix_using(prefix, |prefix| {
        sweep_prefix_until_clear(prefix, SWEEP_GRACE)
    })
    .await
}

/// [`sweep_prefix`] with the blocking sweep `sweep` (test seam).
pub(crate) async fn sweep_prefix_using(
    prefix: PathBuf,
    sweep: impl FnOnce(&Path) -> PrefixSweep + Send + 'static,
) -> PrefixSweep {
    match tokio::task::spawn_blocking(move || sweep(&prefix)).await {
        Ok(sweep) => sweep,
        Err(error) => {
            tracing::warn!(%error, "prefix sweep task failed");
            PrefixSweep::Incomplete { survivors: 0 }
        }
    }
}

/// The console line reporting processes the sweep could not end, if any
/// (no paths: the line reaches the UI).
pub(crate) fn survivors_notice(sweep: PrefixSweep) -> Option<String> {
    match sweep {
        PrefixSweep::Incomplete { survivors: 0 } => {
            Some("Could not check the prefix for leftover processes.".to_string())
        }
        PrefixSweep::Incomplete { survivors } => Some(format!(
            "{survivors} process(es) still use the prefix after the run ended."
        )),
        PrefixSweep::Clear | PrefixSweep::Skipped => None,
    }
}

/// [`sweep_prefix`] after a cancel or stop, with the notice (if any) logged
/// to the job's console.
pub(crate) async fn sweep_after_cancel(ctx: &JobContext, prefix: PathBuf) -> PrefixSweep {
    let sweep = sweep_prefix(prefix).await;
    log_survivors(ctx, sweep);
    sweep
}

/// Logs [`survivors_notice`] of `sweep`, if any, to the job's console.
pub(crate) fn log_survivors(ctx: &JobContext, sweep: PrefixSweep) {
    if let Some(notice) = survivors_notice(sweep) {
        ctx.log(vec![notice]);
    }
}

/// Whether a child that ran was cancelled or killed, or its supervision
/// failed (its fate is unknown): the prefix must then be swept.
pub(crate) fn child_was_stopped(result: &Result<ChildExit, ChildError>) -> bool {
    match result {
        Ok(exit) => exit.was_cancelled(),
        Err(ChildError::Supervise(_)) => true,
        Err(ChildError::Cancelled | ChildError::Spawn(_)) => false,
    }
}
