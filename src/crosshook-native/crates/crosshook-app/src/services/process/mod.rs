//! Shared plumbing for the services that run one host process as a job:
//! install-game, update-game, run-executable and prefix-dependency installs.
//!
//! - [`run::start_run`] starts the job and hands back a [`ProcessRun`] only
//!   once the body signalled `started`, so no output precedes the handle.
//! - [`child::run_child`] spawns, supervises, tees and tails one child, and
//!   returns only after the final log drain, so a completion sent afterwards
//!   always follows the last log line.
//! - [`owner::ActiveJobs`] maps an invoking owner to its active job for the
//!   owner-checked cancel/stop commands (D6).
//! - Every prefix-touching job holds [`prefix_job_key`] (D5).

pub(crate) mod child;
pub(crate) mod owner;
pub(crate) mod run;
pub(crate) mod tee;

use std::path::Path;

use crosshook_core::prefix_deps::runner::normalize_prefix_path;

use crate::jobs::JobKey;
use crate::{AppError, AppErrorKind};

pub use owner::JobOwner;
pub use run::{ProcessRun, RunEvent};

/// Prefix of the shared per-prefix exclusivity key.
pub(crate) const PREFIX_KEY_PREFIX: &str = "prefix:";

/// Conflict text for a second job on a busy prefix.
pub(crate) const MSG_PREFIX_BUSY: &str = "another operation is already running for this prefix";

/// Normalizes `raw_prefix` to its canonical effective prefix (the `pfx`
/// directory when present) and returns it with its exclusivity key.
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

/// The exclusivity key of an already canonical prefix path.
pub(crate) fn prefix_job_key_for_path(canonical: &Path) -> JobKey {
    JobKey::new(format!(
        "{PREFIX_KEY_PREFIX}{}",
        canonical.to_string_lossy()
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

#[cfg(test)]
mod tests;
