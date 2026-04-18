#![cfg(test)]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use rusqlite::params;
use tempfile::tempdir;

use super::test_support::{connection, sample_profile};
use super::{MetadataStore, MetadataStoreError, SyncSource};
use crate::profile::ProfileStore;

#[test]
fn test_observe_profile_write_creates_row() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let conn = connection(&store);
    let (profile_id, current_filename, game_name, launch_method): (
        String,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT profile_id, current_filename, game_name, launch_method FROM profiles WHERE current_filename = ?1",
            params!["elden-ring"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    assert!(!profile_id.trim().is_empty());
    assert_eq!(current_filename, "elden-ring");
    assert_eq!(game_name.as_deref(), Some("Elden Ring"));
    assert_eq!(launch_method.as_deref(), Some("steam_applaunch"));
}

#[test]
fn test_observe_profile_write_idempotent() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();
    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM profiles WHERE current_filename = ?1",
            params!["elden-ring"],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(row_count, 1);
}

#[test]
fn test_observe_profile_rename_creates_history() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let old_path = std::path::Path::new("/profiles/old-name.toml");
    let new_path = std::path::Path::new("/profiles/new-name.toml");

    store
        .observe_profile_write("old-name", &profile, old_path, SyncSource::AppWrite, None)
        .unwrap();
    store
        .observe_profile_rename("old-name", "new-name", old_path, new_path)
        .unwrap();

    let conn = connection(&store);
    let renamed_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM profiles WHERE current_filename = ?1",
            params!["new-name"],
            |row| row.get(0),
        )
        .unwrap();
    let history: (String, String, String, String) = conn
        .query_row(
            "SELECT old_name, new_name, old_path, new_path FROM profile_name_history",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    assert_eq!(renamed_count, 1);
    assert_eq!(history.0, "old-name");
    assert_eq!(history.1, "new-name");
    assert_eq!(history.2, old_path.to_string_lossy());
    assert_eq!(history.3, new_path.to_string_lossy());
}

#[test]
fn test_observe_profile_delete_tombstones() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();
    store.observe_profile_delete("elden-ring").unwrap();

    let conn = connection(&store);
    let (row_count, deleted_at): (i64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*), MAX(deleted_at) FROM profiles WHERE current_filename = ?1",
            params!["elden-ring"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(row_count, 1);
    assert!(deleted_at.is_some());
}

#[test]
fn test_sync_profiles_from_store() {
    let temp_dir = tempdir().unwrap();
    let store = MetadataStore::open_in_memory().unwrap();
    let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    profile_store.save("alpha", &profile).unwrap();
    profile_store.save("beta", &profile).unwrap();
    profile_store.save("gamma", &profile).unwrap();

    let report = store.sync_profiles_from_store(&profile_store).unwrap();

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
        .unwrap();

    assert_eq!(report.profiles_seen, 3);
    assert_eq!(report.created, 3);
    assert_eq!(report.updated, 0);
    assert_eq!(report.deleted, 0);
    assert!(report.errors.is_empty());
    assert_eq!(row_count, 3);
}

#[test]
fn test_unavailable_store_noop() {
    let temp_dir = tempdir().unwrap();
    let store = MetadataStore::disabled();
    let profile = sample_profile();
    let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

    assert!(store
        .observe_profile_write(
            "elden-ring",
            &profile,
            std::path::Path::new("/profiles/elden-ring.toml"),
            SyncSource::AppWrite,
            None,
        )
        .is_ok());
    assert!(store
        .observe_profile_rename(
            "elden-ring",
            "elden-ring-renamed",
            std::path::Path::new("/profiles/elden-ring.toml"),
            std::path::Path::new("/profiles/elden-ring-renamed.toml"),
        )
        .is_ok());
    assert!(store.observe_profile_delete("elden-ring").is_ok());

    let report = store.sync_profiles_from_store(&profile_store).unwrap();
    assert_eq!(report.profiles_seen, 0);
    assert_eq!(report.created, 0);
    assert_eq!(report.updated, 0);
    assert_eq!(report.deleted, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn test_file_permissions() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("metadata.db");

    let _store = MetadataStore::with_path(&db_path).unwrap();

    let mode = fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn test_symlink_rejected() {
    let temp_dir = tempdir().unwrap();
    let target_path = temp_dir.path().join("real-metadata.db");
    let symlink_path = temp_dir.path().join("metadata.db");

    fs::write(&target_path, b"").unwrap();
    symlink(&target_path, &symlink_path).unwrap();

    let error = match MetadataStore::with_path(&symlink_path) {
        Ok(_) => panic!("expected metadata symlink path to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, MetadataStoreError::SymlinkDetected(path) if path == symlink_path));
}
