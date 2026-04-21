//! Flatpak first-run migration: import host AppImage data into the sandbox on first launch.
//!
//! Decision gate lives in `src-tauri/src/lib.rs`: when running under Flatpak and
//! `CROSSHOOK_FLATPAK_HOST_XDG` is unset, `run()` is invoked before any store's
//! `BaseDirs::new()` call to populate the sandbox tree from the host AppImage tree (one-way, idempotent).

mod copier;
mod detector;
mod prefix_root;
mod types;

pub use prefix_root::host_prefix_root;
pub use types::{
    FlatpakMigrationError, MigrationOutcome, CONFIG_ROOT_SEGMENT, DATA_INCLUDE_FILES,
    DATA_INCLUDE_SUBTREES, DATA_SKIP_SUBTREES,
};

use prefix_root::is_isolation_mode_active;

use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::platform::{is_flatpak, SystemEnv};

/// Returns `true` when the user has explicitly opted into Flatpak host-XDG
/// shared mode (the legacy Phase 1 behavior) via `CROSSHOOK_FLATPAK_HOST_XDG`.
///
/// Accepts `"1"` or any case variant of `"true"`, trimmed. Anything else —
/// including unset, `"0"`, `"false"`, or empty — leaves per-app isolation
/// (the default) active. Inverse of the crate-internal
/// [`is_isolation_mode_active`] helper; provided as a public entry point so
/// thin-IPC callers (e.g. `src-tauri/src/lib.rs`) don't have to re-implement
/// the parsing.
pub fn is_host_xdg_opt_in() -> bool {
    !is_isolation_mode_active(&SystemEnv)
}

/// Run the Flatpak first-run migration.
///
/// - When `platform::is_flatpak() == false`, returns `Ok(MigrationOutcome::default())` (no-op).
/// - Otherwise, derives host paths from `HOME` and sandbox paths from `BaseDirs::new()`, and
///   imports host config verbatim + selective data subtrees. Idempotent: subsequent calls find
///   the sandbox populated and short-circuit.
///
/// Failure policy: continue-on-error. One subtree failing does not abort the rest.
pub fn run() -> Result<MigrationOutcome, FlatpakMigrationError> {
    if !is_flatpak() {
        return Ok(MigrationOutcome::default());
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(FlatpakMigrationError::HomeDirectoryUnavailable)?;

    let base_dirs =
        directories::BaseDirs::new().ok_or(FlatpakMigrationError::HomeDirectoryUnavailable)?;

    let sandbox_config_root = base_dirs.config_dir().join(CONFIG_ROOT_SEGMENT);
    let sandbox_data_root = base_dirs.data_local_dir().to_path_buf();

    run_impl(&home, &sandbox_config_root, &sandbox_data_root)
}

/// Test seam: run the migration with explicit host and sandbox roots for deterministic unit and
/// integration tests. Kept always-compiled (not `#[cfg(test)]`) so integration tests in
/// `crates/*/tests/` can call it across the crate boundary. The `#[doc(hidden)]` attribute
/// prevents it from appearing in public documentation.
#[doc(hidden)]
pub fn run_for_roots(
    host_home: &Path,
    sandbox_config_root: &Path,
    sandbox_data_root: &Path,
) -> Result<MigrationOutcome, FlatpakMigrationError> {
    run_impl(host_home, sandbox_config_root, sandbox_data_root)
}

fn run_impl(
    host_home: &Path,
    sandbox_config_root: &Path,
    sandbox_data_root: &Path,
) -> Result<MigrationOutcome, FlatpakMigrationError> {
    let host_config = detector::host_config_dir(host_home);
    // host_data_dir returns `<home>/.local/share/crosshook`; copy_data_subtrees expects the
    // parent `.local/share` dir (relative paths like `crosshook/community` are joined onto it).
    let host_data = detector::host_data_dir(host_home);
    // host_data_dir always returns `<home>/.local/share/crosshook` (4 components),
    // so `.parent()` is always `Some`; the expect is an invariant assertion.
    let host_data_parent = host_data
        .parent()
        .expect("host_data_dir always has a parent (.local/share)")
        .to_path_buf();

    let mut outcome = MigrationOutcome::default();

    // Step 1: config root. Copy whole tree if sandbox config is empty and host config exists.
    match detector::needs_first_run(sandbox_config_root, &host_config) {
        Ok(true) => match copier::copy_tree_or_rollback(&host_config, sandbox_config_root) {
            Ok(()) => {
                outcome.imported_config = true;
                info!(
                    host = %host_config.display(),
                    sandbox = %sandbox_config_root.display(),
                    "imported flatpak host config"
                );
                eprintln!("CrossHook: imported host config into sandbox");
            }
            Err(err) => {
                warn!(error = %err, "flatpak config copy failed");
                eprintln!("CrossHook: flatpak config copy failed: {err}");
                // Config copy failure is logged but not fatal: data subtrees may still succeed.
            }
        },
        Ok(false) => {
            // No migration needed: host is empty or sandbox is already populated.
        }
        Err(err) => {
            warn!(error = %err, "flatpak config detection failed");
            eprintln!("CrossHook: flatpak config detection failed: {err}");
        }
    }

    // Step 2: selective data subtrees. Continue-on-error: all failures are collected.
    // We intentionally report the configured skip-policy (`DATA_SKIP_SUBTREES`)
    // on `outcome.skipped_subtrees` rather than `_skipped_existed` (the subset
    // that actually existed on the host). Upstream consumers care about what
    // the policy *is* so they can explain the isolation contract to users;
    // the per-run actuals would add variability without actionable signal.
    let (imported, _skipped_existed, errors) =
        copier::copy_data_subtrees(&host_data_parent, sandbox_data_root);
    outcome.imported_subtrees = imported;
    outcome.skipped_subtrees = DATA_SKIP_SUBTREES.to_vec();

    for err in &errors {
        warn!(error = %err, "flatpak data subtree import error (continuing)");
        eprintln!("CrossHook: flatpak data subtree import error: {err}");
    }

    if outcome.imported_config || !outcome.imported_subtrees.is_empty() {
        info!(
            imported_config = outcome.imported_config,
            imported_subtrees = outcome.imported_subtrees.len(),
            errors = errors.len(),
            "flatpak first-run migration summary"
        );
    }

    Ok(outcome)
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
    fn full_import_then_idempotent_second_run() {
        let home = tempdir().unwrap();
        let sandbox_config = tempdir().unwrap();
        let sandbox_data = tempdir().unwrap();

        // Populate host config (note: detector::host_config_dir returns <home>/.config/crosshook)
        write(
            &home.path().join(".config/crosshook/settings.toml"),
            b"[app]\n",
        );
        // Populate host data include subtrees
        write(
            &home
                .path()
                .join(".local/share/crosshook/community/taps/a/README.md"),
            b"r",
        );
        write(
            &home.path().join(".local/share/crosshook/media/cover.png"),
            b"i",
        );
        // Populate metadata trio
        write(
            &home.path().join(".local/share/crosshook/metadata.db"),
            b"db",
        );
        write(
            &home.path().join(".local/share/crosshook/metadata.db-wal"),
            b"wal",
        );
        // Populate a skip subtree — must NOT appear in sandbox
        write(
            &home
                .path()
                .join(".local/share/crosshook/prefixes/ex/drive_c/x"),
            b"x",
        );

        // sandbox_config_root is the inner crosshook/ dir, matching BaseDirs shape.
        let sandbox_config_inner = sandbox_config.path().join("crosshook");
        let outcome =
            run_for_roots(home.path(), &sandbox_config_inner, sandbox_data.path()).unwrap();

        assert!(outcome.imported_config, "config should import");
        assert!(sandbox_config_inner.join("settings.toml").exists());
        assert!(sandbox_data
            .path()
            .join("crosshook/community/taps/a/README.md")
            .exists());
        assert!(sandbox_data.path().join("crosshook/metadata.db").exists());
        assert!(sandbox_data
            .path()
            .join("crosshook/metadata.db-wal")
            .exists());
        assert!(
            !sandbox_data.path().join("crosshook/prefixes").exists(),
            "skip subtree must not materialize"
        );

        // Second run — must be fully idempotent.
        let outcome2 =
            run_for_roots(home.path(), &sandbox_config_inner, sandbox_data.path()).unwrap();
        assert!(
            !outcome2.imported_config,
            "second run must not re-import config"
        );
        assert!(
            outcome2.imported_subtrees.is_empty(),
            "second run must not re-import subtrees"
        );
    }

    #[test]
    fn host_missing_is_noop() {
        let home = tempdir().unwrap();
        let sandbox_config = tempdir().unwrap();
        let sandbox_data = tempdir().unwrap();
        let sandbox_config_inner = sandbox_config.path().join("crosshook");

        let outcome =
            run_for_roots(home.path(), &sandbox_config_inner, sandbox_data.path()).unwrap();
        assert!(!outcome.imported_config);
        assert!(outcome.imported_subtrees.is_empty());
    }

    #[test]
    fn partial_subtree_present_on_host() {
        let home = tempdir().unwrap();
        let sandbox_config = tempdir().unwrap();
        let sandbox_data = tempdir().unwrap();
        let sandbox_config_inner = sandbox_config.path().join("crosshook");

        // Config present, only media subtree populated, no metadata.db
        write(&home.path().join(".config/crosshook/settings.toml"), b"x");
        write(
            &home.path().join(".local/share/crosshook/media/logo.png"),
            b"p",
        );

        let outcome =
            run_for_roots(home.path(), &sandbox_config_inner, sandbox_data.path()).unwrap();
        assert!(outcome.imported_config);
        assert_eq!(outcome.imported_subtrees, vec!["crosshook/media"]);
        assert!(!sandbox_data.path().join("crosshook/community").exists());
    }
}
