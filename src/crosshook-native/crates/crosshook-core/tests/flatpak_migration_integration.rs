//! End-to-end integration test for Flatpak first-run migration.
//!
//! Uses `flatpak_migration::run_for_roots` (a `#[doc(hidden)] pub` test seam) to
//! exercise the full pipeline against synthetic host/sandbox tempdirs. Covers
//! the include/skip matrix from the plan's Testing Strategy section.

use std::fs;
use std::path::Path;

use crosshook_core::flatpak_migration::{run_for_roots, MigrationOutcome};
use tempfile::TempDir;

fn write(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write fixture file");
}

fn populate_host_fixture(home: &Path) {
    // Config (fully copied)
    write(
        &home.join(".config/crosshook/settings.toml"),
        b"[app]\ntheme = \"dark\"\n",
    );
    write(
        &home.join(".config/crosshook/profiles/example.toml"),
        b"[profile]\nname = \"example\"\n",
    );

    // Data: include subtrees
    write(
        &home.join(".local/share/crosshook/community/taps/example/README.md"),
        b"# example\n",
    );
    write(&home.join(".local/share/crosshook/media/cover.png"), &[]);
    write(
        &home.join(".local/share/crosshook/launchers/custom.sh"),
        b"#!/bin/sh\n",
    );

    // Data: include files (metadata DB trio) — write raw bytes; no need for rusqlite
    // since we assert file presence, not schema validity.
    write(
        &home.join(".local/share/crosshook/metadata.db"),
        b"SQLite format 3\0",
    );
    write(&home.join(".local/share/crosshook/metadata.db-wal"), b"wal");
    write(&home.join(".local/share/crosshook/metadata.db-shm"), b"shm");

    // Data: skip subtrees (MUST NOT be copied)
    write(
        &home.join(".local/share/crosshook/prefixes/example/drive_c/placeholder"),
        b"do-not-copy",
    );
    write(
        &home.join(".local/share/crosshook/artifacts/logfile.log"),
        b"l",
    );
    write(&home.join(".local/share/crosshook/cache/tmp.bin"), b"c");
    write(&home.join(".local/share/crosshook/logs/ch.log"), b"l");
    write(
        &home.join(".local/share/crosshook/runtime-helpers/helper.sh"),
        b"#!/bin/sh\n",
    );
}

fn run_migration(
    host: &Path,
    sandbox_config_inner: &Path,
    sandbox_data: &Path,
) -> MigrationOutcome {
    run_for_roots(host, sandbox_config_inner, sandbox_data)
        .expect("migration should succeed on good fixtures")
}

#[test]
fn full_import_includes_expected_items() {
    let home = TempDir::new().unwrap();
    let sandbox_config = TempDir::new().unwrap();
    let sandbox_data = TempDir::new().unwrap();
    populate_host_fixture(home.path());

    let sandbox_config_inner = sandbox_config.path().join("crosshook");
    let outcome = run_migration(home.path(), &sandbox_config_inner, sandbox_data.path());

    // Config imported
    assert!(outcome.imported_config);
    assert!(sandbox_config_inner.join("settings.toml").exists());
    assert!(sandbox_config_inner.join("profiles/example.toml").exists());

    // Include subtrees
    for entry in &[
        "crosshook/community",
        "crosshook/media",
        "crosshook/launchers",
    ] {
        assert!(
            sandbox_data.path().join(entry).exists(),
            "include subtree missing in sandbox: {entry}"
        );
        assert!(
            outcome.imported_subtrees.contains(entry),
            "imported_subtrees should contain {entry}, got {:?}",
            outcome.imported_subtrees,
        );
    }

    // Metadata trio
    for entry in &[
        "crosshook/metadata.db",
        "crosshook/metadata.db-wal",
        "crosshook/metadata.db-shm",
    ] {
        assert!(
            sandbox_data.path().join(entry).exists(),
            "metadata file missing in sandbox: {entry}"
        );
    }

    // Skip subtrees — MUST NOT exist in sandbox
    for entry in &[
        "crosshook/prefixes",
        "crosshook/artifacts",
        "crosshook/cache",
        "crosshook/logs",
        "crosshook/runtime-helpers",
    ] {
        assert!(
            !sandbox_data.path().join(entry).exists(),
            "skip subtree MUST NOT exist in sandbox: {entry}"
        );
    }
}

#[test]
fn second_run_is_idempotent() {
    let home = TempDir::new().unwrap();
    let sandbox_config = TempDir::new().unwrap();
    let sandbox_data = TempDir::new().unwrap();
    populate_host_fixture(home.path());

    let sandbox_config_inner = sandbox_config.path().join("crosshook");
    let first = run_migration(home.path(), &sandbox_config_inner, sandbox_data.path());
    assert!(first.imported_config);
    assert!(!first.imported_subtrees.is_empty());

    let second = run_migration(home.path(), &sandbox_config_inner, sandbox_data.path());
    assert!(
        !second.imported_config,
        "second run must not re-import config"
    );
    assert!(
        second.imported_subtrees.is_empty(),
        "second run must not re-import subtrees, got: {:?}",
        second.imported_subtrees
    );
}

#[test]
fn no_migration_when_host_empty() {
    let home = TempDir::new().unwrap();
    let sandbox_config = TempDir::new().unwrap();
    let sandbox_data = TempDir::new().unwrap();
    // host empty — do not populate

    let sandbox_config_inner = sandbox_config.path().join("crosshook");
    let outcome = run_migration(home.path(), &sandbox_config_inner, sandbox_data.path());
    assert!(!outcome.imported_config);
    assert!(outcome.imported_subtrees.is_empty());
}

#[test]
fn partial_host_tree_imports_only_present_entries() {
    let home = TempDir::new().unwrap();
    let sandbox_config = TempDir::new().unwrap();
    let sandbox_data = TempDir::new().unwrap();

    // Only config + media (no metadata.db, no community, no launchers)
    write(
        &home.path().join(".config/crosshook/settings.toml"),
        b"[app]\n",
    );
    write(
        &home.path().join(".local/share/crosshook/media/cover.png"),
        &[],
    );

    let sandbox_config_inner = sandbox_config.path().join("crosshook");
    let outcome = run_migration(home.path(), &sandbox_config_inner, sandbox_data.path());

    assert!(outcome.imported_config);
    assert_eq!(outcome.imported_subtrees, vec!["crosshook/media"]);
    assert!(!sandbox_data.path().join("crosshook/community").exists());
    assert!(!sandbox_data.path().join("crosshook/metadata.db").exists());
}

/// F016 regression: when `.db` copies successfully but `.db-wal` copy fails (simulated via a
/// pre-existing unwritable destination file), the atomic trio rollback removes the already-copied
/// `.db` so the sandbox is left clean, the migration still completes (`run_for_roots` uses
/// continue-on-error), and the returned outcome does NOT list `crosshook/metadata.db` as
/// imported.
///
/// This pins the invariant introduced by the F002 resolution: idempotency skips all three if any
/// dst member exists; copy rolls back any trio members written so far on failure.
#[cfg(unix)]
#[test]
fn wal_trio_partial_failure_rolls_back_and_migration_continues() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempDir::new().unwrap();
    let sandbox_config = TempDir::new().unwrap();
    let sandbox_data = TempDir::new().unwrap();

    // Host: full metadata DB trio + one include subtree so migration has work to do.
    write(
        &home.path().join(".local/share/crosshook/metadata.db"),
        b"SQLite format 3\0",
    );
    write(
        &home.path().join(".local/share/crosshook/metadata.db-wal"),
        b"wal",
    );
    write(
        &home.path().join(".local/share/crosshook/metadata.db-shm"),
        b"shm",
    );
    write(
        &home.path().join(".local/share/crosshook/launchers/run.sh"),
        b"#!/bin/sh\n",
    );

    // Pre-create the sandbox `.db-wal` destination as an unwritable file so that
    // `fs::copy(src_wal, dst_wal)` fails with a permission error after `.db` has
    // already been copied.
    let dst_wal = sandbox_data.path().join("crosshook/metadata.db-wal");
    fs::create_dir_all(dst_wal.parent().unwrap()).expect("create sandbox crosshook dir");
    fs::write(&dst_wal, b"blocker").expect("write blocker file");
    fs::set_permissions(&dst_wal, fs::Permissions::from_mode(0o000))
        .expect("make dst-wal unwritable");

    let sandbox_config_inner = sandbox_config.path().join("crosshook");

    // Migration must succeed overall (continue-on-error semantics).
    let outcome = run_for_roots(home.path(), &sandbox_config_inner, sandbox_data.path())
        .expect("run_for_roots must return Ok even when trio copy fails");

    // Restore permissions so TempDir can clean up the blocker file.
    fs::set_permissions(&dst_wal, fs::Permissions::from_mode(0o644))
        .expect("restore dst-wal permissions for cleanup");

    // The trio must NOT be listed as imported — the failed copy rolled back.
    assert!(
        !outcome.imported_subtrees.contains(&"crosshook/metadata.db"),
        "trio must not be listed as imported after rollback; got {:?}",
        outcome.imported_subtrees,
    );

    // The `.db` file must have been rolled back — it must NOT exist in the sandbox.
    assert!(
        !sandbox_data.path().join("crosshook/metadata.db").exists(),
        "rolled-back .db must not persist in sandbox"
    );

    // The unrelated subtree (launchers) must still have been imported — continue-on-error.
    assert!(
        outcome.imported_subtrees.contains(&"crosshook/launchers"),
        "launchers subtree must still import despite trio failure; got {:?}",
        outcome.imported_subtrees,
    );
}

/// F015 regression: a dangling symlink inside a host include-subtree (plausible for a
/// partially-deleted community tap) must be faithfully preserved in the sandbox. The migration
/// should complete without error and the symlink must be present at the destination.
///
/// `copy_dir_recursive` uses `symlink_metadata()` to detect symlinks and recreates them verbatim
/// via `copy_symlink`; a dangling target does not prevent the link from being created.
#[cfg(unix)]
#[test]
fn dangling_symlink_in_include_subtree_is_preserved() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let sandbox_config = TempDir::new().unwrap();
    let sandbox_data = TempDir::new().unwrap();

    // Populate the community subtree with a real file and a dangling symlink that
    // points to a path that does not exist.
    let tap_dir = home
        .path()
        .join(".local/share/crosshook/community/taps/demo");
    fs::create_dir_all(&tap_dir).expect("create tap dir");
    write(&tap_dir.join("README.md"), b"# demo\n");
    symlink("missing-target.md", tap_dir.join("stale-link.md")).expect("create dangling symlink");

    let sandbox_config_inner = sandbox_config.path().join("crosshook");
    let outcome = run_migration(home.path(), &sandbox_config_inner, sandbox_data.path());

    // Migration must complete without error.
    assert!(
        outcome.imported_subtrees.contains(&"crosshook/community"),
        "community subtree must be imported; got {:?}",
        outcome.imported_subtrees,
    );

    let dst_tap = sandbox_data.path().join("crosshook/community/taps/demo");

    // The real file must be present.
    assert!(
        dst_tap.join("README.md").exists(),
        "README.md must be copied into sandbox"
    );

    // The dangling symlink must be present as a symlink in the sandbox.
    let dst_link = dst_tap.join("stale-link.md");
    assert!(
        dst_link.symlink_metadata().is_ok(),
        "dangling symlink must exist in sandbox (as a symlink entry)"
    );
    assert!(
        dst_link.is_symlink(),
        "sandbox entry for stale-link.md must remain a symlink"
    );
    // Confirm the target is still the same dangling path.
    assert_eq!(
        fs::read_link(&dst_link).unwrap(),
        std::path::PathBuf::from("missing-target.md"),
    );
    // Confirm it genuinely dangles — following the link must fail.
    assert!(
        !dst_link.exists(),
        "dangling symlink must not resolve to an existing file"
    );
}
