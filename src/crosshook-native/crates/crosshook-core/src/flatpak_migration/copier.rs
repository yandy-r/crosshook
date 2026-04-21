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

    // --- Metadata DB trio (atomic: .db + optional .db-wal + optional .db-shm) ---
    //
    // SQLite's WAL protocol requires the `.db` file and the companion `.db-wal` /
    // `.db-shm` files to refer to the same database state. Copying them per-file
    // with independent idempotency checks can leave the sandbox with a stale
    // `.db` paired with a fresher `.db-wal`, which SQLite treats as a journal-
    // replay mismatch. Treat the trio as one unit:
    //
    //   * Idempotency: if ANY trio member is already present in the sandbox,
    //     skip the whole trio (the DB is considered populated).
    //   * Copy order: `.db` first (guaranteed by `DATA_INCLUDE_FILES` ordering),
    //     then `.db-wal` and `.db-shm` if the host has them.
    //   * Rollback on failure: any trio member already copied in this run is
    //     removed so the next retry starts from a clean sandbox.
    //
    // The trio reports as a single `imported` entry (the `.db` path) so the
    // frontend toast count reflects "one database imported" rather than three
    // raw files.
    let trio_dsts: Vec<PathBuf> = DATA_INCLUDE_FILES
        .iter()
        .map(|entry| sandbox_data_root.join(entry))
        .collect();
    let trio_srcs: Vec<PathBuf> = DATA_INCLUDE_FILES
        .iter()
        .map(|entry| host_data_root.join(entry))
        .collect();

    if trio_dsts.iter().any(|d| d.exists()) {
        debug!("sandbox metadata db trio already present, skipping atomic copy");
    } else if !trio_srcs.first().is_some_and(|db| db.exists()) {
        // No host .db → nothing to import. Any orphan wal/shm on the host is
        // meaningless without its journal and is deliberately ignored.
        debug!("host metadata.db absent, nothing to import");
    } else {
        let mut copied: Vec<PathBuf> = Vec::new();
        let mut trio_err: Option<FlatpakMigrationError> = None;
        for (entry, (src, dst)) in DATA_INCLUDE_FILES
            .iter()
            .zip(trio_srcs.iter().zip(trio_dsts.iter()))
        {
            if !src.exists() {
                debug!(file = *entry, "host trio member absent, skipping member");
                continue;
            }
            if let Some(parent) = dst.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    trio_err = Some(FlatpakMigrationError::Io {
                        path: parent.to_path_buf(),
                        source: err,
                    });
                    break;
                }
            }
            match fs::copy(src, dst) {
                Ok(_) => copied.push(dst.clone()),
                Err(err) => {
                    trio_err = Some(FlatpakMigrationError::Io {
                        path: dst.clone(),
                        source: err,
                    });
                    break;
                }
            }
        }
        if let Some(err) = trio_err {
            for path in &copied {
                let _ = fs::remove_file(path);
            }
            warn!(error = %err, "metadata db trio copy failed, rolled back");
            eprintln!("CrossHook: metadata db trio copy failed: {err}");
            errors.push(err);
        } else if !copied.is_empty() {
            // Representative entry: the `.db` path. One logical import, not three.
            imported.push(DATA_INCLUDE_FILES[0]);
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
        // Atomic trio: reports once via the representative `.db` entry, but
        // both .db and .db-wal land on disk.
        assert!(imported.contains(&"crosshook/metadata.db"));
        assert!(!imported.contains(&"crosshook/metadata.db-wal"));
        assert!(sandbox.join("crosshook/metadata.db").exists());
        assert!(sandbox.join("crosshook/metadata.db-wal").exists());
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
        // Trio reports as one logical entry (the representative `.db`), but all
        // three files physically land in the sandbox.
        assert_eq!(
            imported
                .iter()
                .filter(|e| e.starts_with("crosshook/metadata.db"))
                .count(),
            1,
            "trio should report once; got {imported:?}"
        );
        assert!(imported.contains(&"crosshook/metadata.db"));
        for name in &[
            "crosshook/metadata.db",
            "crosshook/metadata.db-wal",
            "crosshook/metadata.db-shm",
        ] {
            assert!(sandbox_root.path().join(name).exists());
        }
    }

    #[test]
    fn copy_data_subtrees_trio_atomic_idempotency_on_partial_sandbox() {
        // Sandbox already has the `.db` (e.g. from a previous run) but no
        // wal/shm. The host has a full trio. Atomic idempotency must skip all
        // three — copying only the host wal/shm on top of a stale sandbox .db
        // would be a SQLite journal-replay corruption risk.
        let host_root = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        write(&host_root.path().join("crosshook/metadata.db"), b"new-db");
        write(
            &host_root.path().join("crosshook/metadata.db-wal"),
            b"new-wal",
        );
        write(
            &host_root.path().join("crosshook/metadata.db-shm"),
            b"new-shm",
        );
        write(
            &sandbox_root.path().join("crosshook/metadata.db"),
            b"old-db",
        );
        let (imported, _, errors) = copy_data_subtrees(host_root.path(), sandbox_root.path());
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(
            !imported.contains(&"crosshook/metadata.db"),
            "partial-sandbox trio must be skipped entirely"
        );
        assert!(!sandbox_root
            .path()
            .join("crosshook/metadata.db-wal")
            .exists());
        assert!(!sandbox_root
            .path()
            .join("crosshook/metadata.db-shm")
            .exists());
        // The pre-existing sandbox .db is untouched.
        assert_eq!(
            fs::read(sandbox_root.path().join("crosshook/metadata.db")).unwrap(),
            b"old-db",
        );
    }

    #[test]
    fn copy_data_subtrees_trio_absent_on_host_is_noop() {
        let host_root = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        // Orphan wal with no .db on host — must be ignored, not imported.
        write(
            &host_root.path().join("crosshook/metadata.db-wal"),
            b"orphan",
        );
        let (imported, _, errors) = copy_data_subtrees(host_root.path(), sandbox_root.path());
        assert!(errors.is_empty());
        assert!(!imported.iter().any(|e| e.starts_with("crosshook/metadata")));
        assert!(!sandbox_root
            .path()
            .join("crosshook/metadata.db-wal")
            .exists());
    }
}
