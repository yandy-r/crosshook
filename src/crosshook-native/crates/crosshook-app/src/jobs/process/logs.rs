//! Log-file creation and target slugs.
//!
//! The private log directory is resolved and verified once at bootstrap
//! (`crosshook_core::runtime_files`); services hold it as a [`LogDir`] and
//! create log files relative to its open handle.

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crosshook_core::runtime_files::{self, PrivateDir, RuntimeDirError};

use crate::{AppError, AppErrorKind};

#[cfg(test)]
mod tests;

/// Text of the error when no private log directory could be opened.
const MSG_NO_LOG_DIR: &str = "no private log directory available";

/// The private launch log directory shared by every service, or the reason it
/// could not be opened.
///
/// [`PrivateDir`] and [`RuntimeDirError`] are not `Clone`, so both sides sit
/// behind an [`Arc`] and the alias clones cheaply. An unusable directory is not
/// fatal to bootstrap: only job starts that need a log fail, through
/// [`require`], with a fixed path-free message (the stored reason is logged,
/// never sent over IPC).
pub type LogDir = Result<Arc<PrivateDir>, Arc<RuntimeDirError>>;

/// Borrows the open log directory.
///
/// # Errors
/// `Io` with "no private log directory available" when the directory could not
/// be opened at bootstrap.
pub(crate) fn require(log_dir: &LogDir) -> Result<&PrivateDir, AppError> {
    log_dir
        .as_deref()
        .map_err(|reason| {
            tracing::warn!(reason = %reason, "job needs a log but the private log directory is unavailable");
            AppError::new(AppErrorKind::Io, MSG_NO_LOG_DIR)
        })
}

/// [`create_log_file_in`] with the handle closed, for callers that only need
/// the path (`LaunchService::prepare`).
///
/// # Errors
/// `Io` when the log file cannot be created (see [`create_log_file_in`]).
pub fn create_log_path_in(
    dir: &PrivateDir,
    prefix: &str,
    target_slug: &str,
) -> Result<PathBuf, AppError> {
    create_log_file_in(dir, prefix, target_slug).map(|(path, _file)| path)
}

/// Creates `<dir>/<prefix>-<slug>-<millis>.log` with mode `0o600` and returns
/// its path together with the open, write-only handle.
///
/// The file is created relative to the verified `dir` handle with exclusive
/// creation and a `-N` uniqueness suffix when the same-millisecond name already
/// exists (see [`runtime_files::create_unique_log_file`]), so two sessions that
/// start within one millisecond never share, and never truncate, one log file.
/// Callers attach child stdio to (clones of) the returned handle instead of
/// reopening the path.
///
/// # Errors
/// `Io` carrying the path-free reason when the clock is before the Unix epoch
/// or the file cannot be created.
pub fn create_log_file_in(
    dir: &PrivateDir,
    prefix: &str,
    target_slug: &str,
) -> Result<(PathBuf, File), AppError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::new(AppErrorKind::Io, error.to_string()))?
        .as_millis();
    runtime_files::create_unique_log_file(dir, &format!("{prefix}-{target_slug}-{timestamp}"))
        .map_err(|error| AppError::new(AppErrorKind::Io, error.to_string()))
}

/// A [`LogDir`] at `<root>/logs` for tests, so no test touches the real
/// runtime or state directory.
#[cfg(test)]
pub(crate) fn test_log_dir(root: &std::path::Path) -> LogDir {
    use crosshook_core::runtime_files::{open_private_dir, CandidateRoot, LogDirCandidate};

    open_private_dir(&LogDirCandidate {
        root: root.to_path_buf(),
        relative: vec!["logs"],
        kind: CandidateRoot::State,
    })
    .map(Arc::new)
    .map_err(Arc::new)
}

/// A [`LogDir`] that failed to open, for tests of the unusable-directory path.
#[cfg(test)]
pub(crate) fn unusable_log_dir() -> LogDir {
    Err(Arc::new(RuntimeDirError::NoUsableDir))
}

/// Longest slug kept, so `<prefix>-<slug>-<millis>-N.log.gamescope.pid` stays
/// well under the 255-byte file name limit.
const MAX_SLUG_CHARS: usize = 64;

/// Lowercases ASCII alphanumerics, maps everything else to `-`, keeps at most
/// [`MAX_SLUG_CHARS`] characters and trims `-`; returns `fallback` when nothing
/// remains.
pub fn slugify_target(name: &str, fallback: &str) -> String {
    let slug: String = name
        .chars()
        .take(MAX_SLUG_CHARS)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
