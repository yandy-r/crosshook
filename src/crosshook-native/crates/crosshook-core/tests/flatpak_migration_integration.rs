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
