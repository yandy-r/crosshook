//! Private runtime files (launch logs and watchdog PID captures).
//!
//! Resolves the per-user private log directory, verifies it without following
//! symlinks, opens log files relative to that verified directory and prunes old
//! logs. Errors are path-free so they can cross an IPC boundary.

use std::path::PathBuf;
use std::{fmt, io};

use nix::errno::Errno;

mod location;
pub(crate) use location::{resolve, RuntimeDirInputs};
pub use location::{CandidateRoot, LogDirCandidate};
mod open;
pub use open::{create_unique_log_file, open_private_append, open_private_read};
mod prune;
pub use prune::{prune_stale, RetentionPolicy};
mod secure_dir;
pub use secure_dir::{open_private_dir, PrivateDir};
#[cfg(test)]
mod tests;

/// Flatpak application id, shared with the app-id migration so there is one source.
pub const FLATPAK_APP_ID: &str = crate::app_id_migration::CURRENT_TAURI_APP_ID_DIR;
/// Subdirectory under the runtime base that holds launch logs.
pub const LOG_SUBDIR: &str = "logs";
/// Subdirectory under the runtime base for native (non-Flatpak) installs.
pub(crate) const NATIVE_SUBDIR: &str = "crosshook";

/// Resolves and opens the private launch log directory for this process.
///
/// Not cached: callers resolve once (bootstrap, CLI).
pub fn launch_log_dir() -> Result<PrivateDir, RuntimeDirError> {
    launch_log_dir_from(&RuntimeDirInputs::detect())
}

/// Opens the first usable private log directory among the candidates of `inputs`.
///
/// Rejected candidates are logged (reason as a field, path only as a field) and
/// skipped; [`RuntimeDirError::NoUsableDir`] is returned when none is usable.
pub(crate) fn launch_log_dir_from(
    inputs: &RuntimeDirInputs,
) -> Result<PrivateDir, RuntimeDirError> {
    for candidate in resolve(inputs) {
        match open_private_dir(&candidate) {
            Ok(dir) => return Ok(dir),
            Err(err) => tracing::warn!(
                reason = %err,
                candidate = %candidate.path().display(),
                "runtime log dir candidate rejected"
            ),
        }
    }
    Err(RuntimeDirError::NoUsableDir)
}

/// Candidate log directory paths for diagnostics, without creating anything.
///
/// Includes the Flatpak location when running natively so logs written by
/// either mode are found.
pub fn launch_log_dir_candidates() -> Vec<PathBuf> {
    launch_log_dir_candidates_from(&RuntimeDirInputs::detect())
}

/// Pure form of [`launch_log_dir_candidates`] for tests.
pub(crate) fn launch_log_dir_candidates_from(inputs: &RuntimeDirInputs) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = resolve(inputs).iter().map(LogDirCandidate::path).collect();
    if !inputs.flatpak {
        let flatpak_inputs = RuntimeDirInputs {
            flatpak: true,
            ..inputs.clone()
        };
        for path in resolve(&flatpak_inputs).iter().map(LogDirCandidate::path) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

/// Why a private runtime directory or file could not be resolved or opened.
///
/// [`std::fmt::Display`] never embeds a filesystem path, so the error can cross
/// an IPC boundary without leaking local layout details.
#[derive(Debug)]
pub enum RuntimeDirError {
    /// No candidate location yielded a usable private directory.
    NoUsableDir,
    /// The directory is a symlink or not a directory.
    NotADirectory,
    /// The directory is owned by another user.
    ForeignOwner,
    /// The file is not a regular file owned by the current user.
    UnsafeFile,
    /// Every candidate file name was already taken.
    NamesExhausted,
    /// Another I/O failure.
    Io(io::Error),
}

impl fmt::Display for RuntimeDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUsableDir => f.write_str("no private log directory available"),
            Self::NotADirectory => f.write_str("log directory is a symlink or not a directory"),
            Self::ForeignOwner => f.write_str("log directory is owned by another user"),
            Self::UnsafeFile => {
                f.write_str("log file is not a regular file owned by the current user")
            }
            Self::NamesExhausted => f.write_str("no free log file name"),
            Self::Io(source) => write!(f, "log file I/O failed: {}", source.kind()),
        }
    }
}

impl std::error::Error for RuntimeDirError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for RuntimeDirError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Errno> for RuntimeDirError {
    fn from(errno: Errno) -> Self {
        match errno {
            Errno::ELOOP | Errno::ENOTDIR => Self::NotADirectory,
            other => Self::Io(io::Error::from_raw_os_error(other as i32)),
        }
    }
}
