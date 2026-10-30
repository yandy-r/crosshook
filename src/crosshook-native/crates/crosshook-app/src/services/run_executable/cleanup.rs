//! Throwaway-prefix removal, run inside the job after the child is gone and
//! before the run's completion, so the prefix key is held until the
//! directory is deleted (D5).
//!
//! Safety rules:
//! 1. Only a path for which [`is_throwaway_prefix_path_in`] holds against the
//!    service's `_run-adhoc` root is ever deleted: a direct, non-empty child
//!    of the root without `..` components. Anything else is refused.
//! 2. Symlinks are never followed. A symlink (or file) found in place of the
//!    prefix is removed as an entry with `remove_file`, leaving its target
//!    alone; a real directory is removed with `std::fs::remove_dir_all`,
//!    which does not follow symlinks inside the tree either.
//! 3. A failed attempt (Wine processes still holding the prefix) is followed
//!    by a kill sweep of processes using the prefix, a short settle, and a
//!    retry; at most two retries. There is no shell `rm` fallback.

use std::io;
use std::path::Path;
use std::time::Duration;

use crosshook_core::run_executable::is_throwaway_prefix_path_in;

use crate::jobs::kill_processes_using_prefix;

/// Settle time before each of the two bounded retries.
const RETRY_SETTLE: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(200)];

/// What [`cleanup_throwaway_prefix`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupOutcome {
    /// The prefix entry was deleted.
    Removed,
    /// The path is not a throwaway prefix under the root; nothing was
    /// touched.
    NotThrowaway,
    /// Nothing existed at the path.
    Missing,
    /// Every attempt failed; the prefix is left for the user to delete.
    Failed,
}

/// Deletes the throwaway prefix `prefix_path` under `adhoc_root`, with a
/// kill sweep and two bounded retries when the first attempt fails.
///
/// Blocking (filesystem work and retry settles); call it on the blocking
/// pool.
pub(crate) fn cleanup_throwaway_prefix(prefix_path: &Path, adhoc_root: &Path) -> CleanupOutcome {
    cleanup_with(
        prefix_path,
        adhoc_root,
        RETRY_SETTLE,
        kill_processes_using_prefix,
    )
}

/// [`cleanup_throwaway_prefix`] with injectable retry settles and sweep.
pub(super) fn cleanup_with(
    prefix_path: &Path,
    adhoc_root: &Path,
    settles: [Duration; 2],
    mut sweep: impl FnMut(&Path),
) -> CleanupOutcome {
    if !is_throwaway_prefix_path_in(prefix_path, adhoc_root) {
        tracing::warn!(
            prefix = %prefix_path.display(),
            "refusing to delete a prefix outside the _run-adhoc namespace root"
        );
        return CleanupOutcome::NotThrowaway;
    }
    match std::fs::symlink_metadata(prefix_path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return CleanupOutcome::Missing;
        }
        _ => {}
    }

    if try_remove(prefix_path) {
        return CleanupOutcome::Removed;
    }
    for settle in settles {
        // Wine processes (wineserver) may still hold the prefix.
        sweep(prefix_path);
        std::thread::sleep(settle);
        if try_remove(prefix_path) {
            return CleanupOutcome::Removed;
        }
    }
    tracing::warn!(
        prefix = %prefix_path.display(),
        "could not remove the throwaway prefix after retries; the user may delete it manually"
    );
    CleanupOutcome::Failed
}

/// One removal attempt without following symlinks; `true` once nothing is
/// left at the path.
fn try_remove(prefix_path: &Path) -> bool {
    let result = match std::fs::symlink_metadata(prefix_path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
        Err(error) => Err(error),
        // `symlink_metadata` reports a symlink as a symlink, never as the
        // directory it points to.
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(prefix_path),
        Ok(_) => std::fs::remove_file(prefix_path),
    };
    match result {
        Ok(()) => {
            tracing::info!(prefix = %prefix_path.display(), "removed throwaway prefix");
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
        Err(error) => {
            tracing::warn!(
                %error,
                prefix = %prefix_path.display(),
                "throwaway prefix removal failed"
            );
            false
        }
    }
}
