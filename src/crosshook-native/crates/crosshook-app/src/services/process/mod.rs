//! Shared plumbing for the services that run one host process as a job:
//! install-game, update-game, run-executable and prefix-dependency installs.
//!
//! - [`run::start_run`] starts the job and hands back a [`ProcessRun`] only
//!   once the body signalled `started`, so no output precedes the handle.
//! - [`child::run_child`] spawns, supervises, tees and tails one child,
//!   signals `started` only after a successful spawn, and returns only after
//!   the final log drain, so a completion sent afterwards always follows the
//!   last log line.
//! - [`owner::ActiveJobs`] maps an invoking owner to its one active run for
//!   the owner-checked cancel/stop commands (D6).
//! - [`owned`] is the shared start/body/finish of an owner-bound run whose
//!   completion is the child's exit code or signal (update, run-executable).
//! - Every prefix-touching job holds [`prefix_job_key`] (D5).

pub(crate) mod child;
pub(crate) mod owned;
pub(crate) mod owner;
pub(crate) mod run;
pub(crate) mod tee;

use std::path::Path;

use crosshook_core::prefix_deps::runner::normalize_prefix_path;

use crate::jobs::JobKey;
use crate::{AppError, AppErrorKind};

pub use owned::ProcessCompletion;
pub use owner::JobOwner;
pub use run::{ProcessRun, RunEvent};

/// Prefix of the shared per-prefix exclusivity key.
pub(crate) const PREFIX_KEY_PREFIX: &str = "prefix:";

/// Conflict text for a second job on a busy prefix.
pub(crate) const MSG_PREFIX_BUSY: &str = "another operation is already running for this prefix";

/// Proton's wine-prefix directory under a compat data path.
const PFX_DIR: &str = "pfx";

/// Normalizes `raw_prefix` to its canonical effective prefix (the `pfx`
/// directory when present) and returns it with its exclusivity key.
///
/// The key is the same for every job kind on one Wine prefix, also before
/// Proton created it: see [`prefix_job_key_for_path`].
///
/// # Errors
/// `Validation { field: "prefix_path" }` carrying the core error text when
/// the prefix does not exist or cannot be canonicalized.
pub(crate) fn prefix_job_key(raw_prefix: &str) -> Result<(String, JobKey), AppError> {
    let canonical = normalize_prefix_path(raw_prefix)
        .map_err(|error| AppError::validation("prefix_path", error.to_string()))?;
    let key = prefix_job_key_for_path(Path::new(&canonical));
    Ok((canonical, key))
}

/// The exclusivity key of an already canonical effective prefix (as
/// returned by [`prefix_job_key`]).
///
/// Proton keeps the Wine prefix in `<compat data>/pfx`. A path named `pfx`
/// is keyed as is; any other path is a compat data root whose `pfx` Proton
/// has not created yet and is keyed on that future `<path>/pfx`. So a job on
/// a fresh root and a later job on its then-existing `pfx` share one key.
pub(crate) fn prefix_job_key_for_path(canonical: &Path) -> JobKey {
    let effective = if canonical.file_name().is_some_and(|name| name == PFX_DIR) {
        canonical.to_path_buf()
    } else {
        canonical.join(PFX_DIR)
    };
    JobKey::new(format!(
        "{PREFIX_KEY_PREFIX}{}",
        effective.to_string_lossy()
    ))
}

/// Replaces the registry's generic `Conflict` text with `text`; every other
/// error kind passes through unchanged.
pub(crate) fn conflict_as(error: AppError, text: &str) -> AppError {
    if error.kind() == AppErrorKind::Conflict {
        AppError::new(AppErrorKind::Conflict, text)
    } else {
        error
    }
}

/// Removes the empty log of a run that never started; a failure is only
/// logged.
pub(crate) fn remove_unused_log(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        tracing::debug!(%error, log = %path.display(), "unused process log could not be removed");
    }
}

#[cfg(test)]
mod tests;
