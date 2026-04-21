//! Selective copy with staged rename and continue-on-error policy.
//!
//! # Staged rename strategy
//!
//! For each subtree being imported, the copy is first written to a sibling
//! staging path (`<dst>.migrating`). Once the copy is complete, the stage is
//! renamed into the final destination. Any prior leftover stage is cleaned up
//! before starting. On failure the stage is removed so subsequent runs find a
//! clean slate.
//!
//! # Per-file caveat (metadata DB)
//!
//! The SQLite WAL trio (`.db`, `.db-wal`, `.db-shm`) lives **inside** the
//! `crosshook/` subtree and is copied file-by-file with [`std::fs::copy`],
//! not through the directory-level staged-rename path. This is safe because
//! the migration runs at startup before `MetadataStore::try_new` opens any
//! handle. The `.db` file is always copied first so WAL and SHM cannot arrive
//! without the journal they belong to.
//!
//! # Continue-on-error
//!
//! One subtree or file failing does **not** abort the others. All errors are
//! collected in a `Vec<FlatpakMigrationError>` and returned to the caller.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

use crate::flatpak_migration::types::{
    FlatpakMigrationError, DATA_INCLUDE_FILES, DATA_INCLUDE_SUBTREES, DATA_SKIP_SUBTREES,
};
use crate::fs_util::{copy_dir_recursive, dir_is_empty};

const STAGE_SUFFIX: &str = ".migrating";

/// Copies `src` into `dst` via a sibling staging path so the final rename is
/// atomic on same-filesystem moves.
///
/// # Errors
///
/// - [`FlatpakMigrationError::SourceMissing`] — `src` does not exist.
/// - [`FlatpakMigrationError::DestinationNotEmpty`] — `dst` already exists
///   and is non-empty (idempotency guard).
/// - [`FlatpakMigrationError::Io`] — any other I/O failure.
pub(crate) fn copy_tree_or_rollback(src: &Path, dst: &Path) -> Result<(), FlatpakMigrationError> {
    if !src.exists() {
        return Err(FlatpakMigrationError::SourceMissing(src.to_path_buf()));
    }
    if dst.exists() {
        match dir_is_empty(dst) {
            Ok(true) => { /* empty dst — OK to overwrite via rename */ }
            Ok(false) => {
                return Err(FlatpakMigrationError::DestinationNotEmpty(
                    dst.to_path_buf(),
                ))
            }
            Err(err) => {
                return Err(FlatpakMigrationError::Io {
                    path: dst.to_path_buf(),
                    source: err,
                })
            }
        }
    }

    // Stage sibling path.
    let stage = staging_path(dst);

    // Clean up any leftover stage from a prior crashed run.
    if stage.exists() {
        if let Err(err) = fs::remove_dir_all(&stage) {
            return Err(FlatpakMigrationError::Io {
                path: stage,
                source: err,
            });
        }
    }

    // Ensure parent of dst exists so we can rename into place.
    if let Some(parent) = dst.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return Err(FlatpakMigrationError::Io {
                path: parent.to_path_buf(),
                source: err,
            });
        }
    }

    if let Err(err) = copy_dir_recursive(src, &stage) {
        let _ = fs::remove_dir_all(&stage);
        return Err(FlatpakMigrationError::Io {
            path: stage,
            source: err,
        });
    }

    // If dst exists and is empty, remove it before rename (rename requires dst
    // not to be present on most filesystems).
    if dst.exists() {
        if let Err(err) = fs::remove_dir(dst) {
            let _ = fs::remove_dir_all(&stage);
            return Err(FlatpakMigrationError::Io {
                path: dst.to_path_buf(),
                source: err,
            });
        }
    }

    if let Err(err) = fs::rename(&stage, dst) {
        // Fallback for EXDEV (cross-filesystem rename): copy then remove stage.
        if err.raw_os_error() == Some(libc_exdev()) {
            if let Err(err2) = copy_dir_recursive(&stage, dst) {
                let _ = fs::remove_dir_all(&stage);
                return Err(FlatpakMigrationError::Io {
                    path: dst.to_path_buf(),
                    source: err2,
                });
            }
            let _ = fs::remove_dir_all(&stage);
            return Ok(());
        }
        let _ = fs::remove_dir_all(&stage);
        return Err(FlatpakMigrationError::Io {
            path: dst.to_path_buf(),
            source: err,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn libc_exdev() -> i32 {
    18
}
#[cfg(not(unix))]
fn libc_exdev() -> i32 {
    -1
}

fn staging_path(dst: &Path) -> PathBuf {
    let mut s = dst.as_os_str().to_os_string();
    s.push(STAGE_SUFFIX);
    PathBuf::from(s)
}

/// Copies the selective data subtrees and files from the host XDG data root
/// into the sandbox XDG data root.
///
/// Both roots should be the **parent** of `crosshook/` (i.e. `~/.local/share`
/// on the host, `$XDG_DATA_HOME` inside the sandbox). All entries in the
/// `DATA_INCLUDE_*` / `DATA_SKIP_*` constants are relative paths like
/// `crosshook/community`.
///
/// Returns `(imported, skipped_existed, errors)` where:
///
/// - `imported` — entries successfully copied (or already present in sandbox).
/// - `skipped_existed` — entries in [`DATA_SKIP_SUBTREES`] that existed on the
///   host but were intentionally not copied.
/// - `errors` — per-entry errors collected with continue-on-error semantics.
pub(crate) fn copy_data_subtrees(
    host_data_root: &Path,
    sandbox_data_root: &Path,
) -> (
    Vec<&'static str>,
    Vec<&'static str>,
    Vec<FlatpakMigrationError>,
) {
    let mut imported: Vec<&'static str> = Vec::new();
    let mut skipped_existed: Vec<&'static str> = Vec::new();
    let mut errors: Vec<FlatpakMigrationError> = Vec::new();

    // --- Subtrees (directories) ---
    for entry in DATA_INCLUDE_SUBTREES {
        let src = host_data_root.join(entry);
        let dst = sandbox_data_root.join(entry);
        if !src.exists() {
            debug!(subtree = entry, "host subtree absent, nothing to import");
            continue;
        }
        // Idempotency: if dst is already populated, skip.
        match dir_is_empty(&dst) {
            Ok(false) => {
                debug!(
                    subtree = entry,
                    "sandbox subtree already populated, skipping"
                );
                continue;
            }
            Ok(true) | Err(_) => { /* dst either empty or missing — proceed */ }
        }
        match copy_tree_or_rollback(&src, &dst) {
            Ok(()) => imported.push(entry),
            Err(err) => {
                warn!(subtree = entry, error = %err, "subtree import failed");
                eprintln!("CrossHook: failed to import {entry}: {err}");
                errors.push(err);
            }
        }
    }

    // --- Files (metadata DB trio: .db first, then wal/shm) ---
    for file_entry in DATA_INCLUDE_FILES {
        let src = host_data_root.join(file_entry);
        let dst = sandbox_data_root.join(file_entry);
        if !src.exists() {
            // For metadata.db-wal/shm it is normal for these to be absent.
            debug!(file = file_entry, "host file absent, nothing to import");
            continue;
        }
        if dst.exists() {
            debug!(file = file_entry, "sandbox file already present, skipping");
            continue;
        }
        if let Some(parent) = dst.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                warn!(file = file_entry, error = %err, "could not create dst parent");
                eprintln!("CrossHook: failed to prepare {file_entry}: {err}");
                errors.push(FlatpakMigrationError::Io {
                    path: parent.to_path_buf(),
                    source: err,
                });
                continue;
            }
        }
        match fs::copy(&src, &dst) {
            Ok(_) => imported.push(file_entry),
            Err(err) => {
                warn!(file = file_entry, error = %err, "file import failed");
                eprintln!("CrossHook: failed to copy {file_entry}: {err}");
                errors.push(FlatpakMigrationError::Io {
                    path: dst,
                    source: err,
                });
            }
        }
    }

    // --- Record skipped subtrees that DID exist on host (for reporting) ---
    for entry in DATA_SKIP_SUBTREES {
        let src = host_data_root.join(entry);
        if src.exists() {
            skipped_existed.push(entry);
        }
    }

    (imported, skipped_existed, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn copy_tree_or_rollback_copies_and_cleans_stage() {
        let src_root = tempdir().unwrap();
        let dst_root = tempdir().unwrap();
        let src = src_root.path().join("from");
        let dst = dst_root.path().join("to");
        write(&src.join("a/b.txt"), b"hello");
        copy_tree_or_rollback(&src, &dst).unwrap();
        assert!(dst.join("a/b.txt").exists());
        assert!(!dst_root.path().join(format!("to{STAGE_SUFFIX}")).exists());
    }

    #[test]
    fn copy_tree_or_rollback_errors_when_dst_non_empty() {
        let src_root = tempdir().unwrap();
        let dst_root = tempdir().unwrap();
        let src = src_root.path().join("from");
        let dst = dst_root.path().join("to");
        write(&src.join("x.txt"), b"x");
        write(&dst.join("existing.txt"), b"y");
        assert!(matches!(
            copy_tree_or_rollback(&src, &dst),
            Err(FlatpakMigrationError::DestinationNotEmpty(_))
        ));
    }

    #[test]
    fn copy_tree_or_rollback_source_missing() {
        let dst_root = tempdir().unwrap();
        let src = dst_root.path().join("never");
        let dst = dst_root.path().join("to");
        assert!(matches!(
            copy_tree_or_rollback(&src, &dst),
            Err(FlatpakMigrationError::SourceMissing(_))
        ));
    }

    #[test]
    fn copy_data_subtrees_copies_include_and_skips_skip() {
        let host_root = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        let host = host_root.path();
        let sandbox = sandbox_root.path();
        // Populate include subtrees
        write(&host.join("crosshook/community/taps/demo/README.md"), b"r");
        write(&host.join("crosshook/media/cover.png"), b"i");
        write(&host.join("crosshook/launchers/custom.sh"), b"#!/bin/sh");
        // Populate metadata trio
        write(&host.join("crosshook/metadata.db"), b"sqlite");
        write(&host.join("crosshook/metadata.db-wal"), b"wal");
        // Populate skip subtrees
        write(&host.join("crosshook/prefixes/example/drive_c/x"), b"x");
        write(&host.join("crosshook/artifacts/log.txt"), b"l");
        let (imported, skipped_existed, errors) = copy_data_subtrees(host, sandbox);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(imported.contains(&"crosshook/community"));
        assert!(imported.contains(&"crosshook/media"));
        assert!(imported.contains(&"crosshook/launchers"));
        assert!(imported.contains(&"crosshook/metadata.db"));
        assert!(imported.contains(&"crosshook/metadata.db-wal"));
        assert!(!sandbox.join("crosshook/prefixes").exists());
        assert!(!sandbox.join("crosshook/artifacts").exists());
        assert!(skipped_existed.contains(&"crosshook/prefixes"));
        assert!(skipped_existed.contains(&"crosshook/artifacts"));
    }

    #[test]
    fn copy_data_subtrees_is_idempotent_on_second_run() {
        let host_root = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        write(&host_root.path().join("crosshook/community/a.txt"), b"a");
        let (imp1, _, errs1) = copy_data_subtrees(host_root.path(), sandbox_root.path());
        assert!(errs1.is_empty());
        assert!(imp1.contains(&"crosshook/community"));
        let (imp2, _, errs2) = copy_data_subtrees(host_root.path(), sandbox_root.path());
        assert!(errs2.is_empty());
        assert!(
            !imp2.contains(&"crosshook/community"),
            "second run should be no-op"
        );
    }

    #[test]
    fn copy_data_subtrees_metadata_db_trio_copied_together() {
        let host_root = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        write(&host_root.path().join("crosshook/metadata.db"), b"db");
        write(&host_root.path().join("crosshook/metadata.db-wal"), b"wal");
        write(&host_root.path().join("crosshook/metadata.db-shm"), b"shm");
        let (imported, _, errors) = copy_data_subtrees(host_root.path(), sandbox_root.path());
        assert!(errors.is_empty());
        for name in &[
            "crosshook/metadata.db",
            "crosshook/metadata.db-wal",
            "crosshook/metadata.db-shm",
        ] {
            assert!(imported.contains(name), "missing {name} in {imported:?}");
            assert!(sandbox_root.path().join(name).exists());
        }
    }
}
