use super::*;
use crate::metadata::MetadataStore;
use std::os::unix::fs::{symlink, PermissionsExt};
use tempfile::tempdir;

fn seed(path: &Path, version: u32) -> Connection {
    let conn = Connection::open(path).unwrap();
    migrations::run_migrations(&conn).unwrap();
    if version < 27 {
        conn.execute_batch("DROP TABLE prefix_version_restore_journal;")
            .unwrap();
    }
    conn.pragma_update(None, "user_version", version).unwrap();
    conn
}

fn schema(path: &Path) -> u32 {
    version(&raw_open(path, true).unwrap()).unwrap()
}

#[test]
fn newer_is_readable_typed_readonly_and_byte_identical() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(seed(&path, 28));
    fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();
    let before = fs::read(&path).unwrap();
    let store = MetadataStore::with_path(&path).unwrap();
    assert_eq!(store.status(), newer_status(28));
    assert!(store.is_available());
    assert_eq!(store.with_sqlite_conn("read", version).unwrap(), 28);
    assert!(matches!(
        store.with_sqlite_conn("write", |conn| {
            conn.execute("CREATE TABLE forbidden(id)", [])?;
            Ok(())
        }),
        Err(MetadataStoreError::ReadOnlyNewerSchema)
    ));
    assert!(matches!(
        store.dismiss_readiness_nag("test", 1),
        Err(MetadataStoreError::ReadOnlyNewerSchema)
    ));
    assert!(store.get_dismissed_readiness_nags().unwrap().is_empty());
    assert!(store
        .get_dismissed_keys("profile", "app")
        .unwrap()
        .is_empty());
    assert!(matches!(
        store.delete_prefix_version_restore("/prefix"),
        Err(MetadataStoreError::ReadOnlyNewerSchema)
    ));
    assert!(backup::files(&path).is_empty());
    drop(store);
    assert_eq!(before, fs::read(&path).unwrap());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn newer_live_wal_reads_uncheckpointed_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let writer = seed(&path, 28);
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    writer
        .execute_batch("CREATE TABLE future_data(value); INSERT INTO future_data VALUES ('live');")
        .unwrap();
    let before = fs::read(&path).unwrap();
    let store = MetadataStore::with_path(&path).unwrap();
    let text: String = store
        .with_sqlite_conn("read WAL", |conn| {
            Ok(conn.query_row("SELECT value FROM future_data", [], |row| row.get(0))?)
        })
        .unwrap();
    assert_eq!(text, "live");
    assert_eq!(before, fs::read(&path).unwrap());
    assert_eq!(store.status(), newer_status(28));
    assert!(backup::files(&path).is_empty());
}

#[test]
fn equal_and_fresh_no_backup_older_has_durable_private_snapshot() {
    for from in [0, 26, 27] {
        let dir = tempdir().unwrap();
        let path = dir.path().join("metadata.db");
        if from != 0 {
            drop(seed(&path, from));
        }
        let store = MetadataStore::with_path(&path).unwrap();
        assert_eq!(store.status(), MetadataStatus::Ok);
        assert_eq!(schema(&path), 27);
        let files = store.backup_files();
        assert_eq!(files.len(), usize::from(from == 26));
        if let Some(backup) = files.first() {
            assert_eq!(schema(backup), 26);
            assert!(fs::metadata(backup).unwrap().len() > 0);
            assert_eq!(
                fs::metadata(backup).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        store.dismiss_readiness_nag("test", 1).unwrap();
    }
}

#[test]
fn backup_failure_stays_readonly_without_migration() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(seed(&path, 26));
    // NAME_MAX leaves room for DB name, not its longer backup name. Works as root.
    let path_long = dir.path().join(format!("{}.db", "m".repeat(235)));
    fs::rename(&path, &path_long).unwrap();
    let path = path_long;
    let before = fs::read(&path).unwrap();
    let store = MetadataStore::with_path(&path).unwrap();
    assert_eq!(
        store.status(),
        MetadataStatus::Disabled {
            reason: "backup failed".into()
        }
    );
    assert!(matches!(
        store.dismiss_readiness_nag("test", 1),
        Err(MetadataStoreError::ReadOnlyDisabled { .. })
    ));
    assert_eq!(before, fs::read(&path).unwrap());
    assert!(backup::files(&path).is_empty());
}

#[test]
fn retention_ignores_symlinks_directories_and_decoys() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    for stamp in ["20260101T000000Z", "20260201T000000Z", "20260301T000000Z"] {
        fs::write(
            path.with_file_name(format!("metadata.db.bak-v26-{stamp}")),
            b"backup",
        )
        .unwrap();
    }
    let decoys = [
        "metadata.db.bak-v26-nope",
        "metadata.db.bak-v026-20260401T000000Z",
        "metadata.db.bak-v26-20260401T000000Z-extra",
    ];
    for name in decoys {
        fs::write(dir.path().join(name), b"decoy").unwrap();
    }
    let link = dir.path().join("metadata.db.bak-v26-20260501T000000Z");
    symlink(dir.path().join(decoys[0]), &link).unwrap();
    let directory = dir.path().join("metadata.db.bak-v26-20260601T000000Z");
    fs::create_dir(&directory).unwrap();
    backup::retain_two(&path);
    assert_eq!(backup::files(&path).len(), 2);
    assert!(!dir
        .path()
        .join("metadata.db.bak-v26-20260101T000000Z")
        .exists());
    for name in decoys {
        assert!(dir.path().join(name).exists());
    }
    assert!(link.is_symlink());
    assert!(directory.is_dir());
}

#[test]
fn schema_status_exact_json_and_extended_readonly_mapping() {
    for (status, json) in [
        (MetadataStatus::Ok, serde_json::json!({"state":"ok"})),
        (
            newer_status(28),
            serde_json::json!({"state":"newer_schema","found":28,"supported":27}),
        ),
        (
            MetadataStatus::Disabled {
                reason: "backup failed".into(),
            },
            serde_json::json!({"state":"disabled","reason":"backup failed"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&status).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<MetadataStatus>(json).unwrap(),
            status
        );
    }
    let error = MetadataStoreError::from(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_READONLY_DBMOVED),
        None,
    ));
    assert!(error.is_read_only_schema());
}

#[test]
fn concurrent_upgrade_after_backup_never_mutates_header() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(seed(&path, 26));
    let mut upgraded = Vec::new();
    let opened = open_with_hooks(
        &path,
        |_, _| {
            let other = Connection::open(&path)?;
            other.pragma_update(None, "user_version", 28)?;
            other.pragma_update(None, "application_id", 123)?;
            drop(other);
            upgraded = fs::read(&path).unwrap();
            Ok(())
        },
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(opened.status, newer_status(28));
    assert_eq!(upgraded, fs::read(&path).unwrap());
    assert!(backup::files(&path).is_empty());
}

#[test]
fn repeated_schema_race_disables_and_discards_snapshots() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(seed(&path, 26));
    let opened = open_with_hooks(
        &path,
        |from, _| {
            let other = Connection::open(&path)?;
            other.pragma_update(None, "user_version", from - 1)?;
            Ok(())
        },
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        opened.status,
        MetadataStatus::Disabled {
            reason: "schema changed during startup".into()
        }
    );
    assert_eq!(schema(&path), 24);
    assert!(backup::files(&path).is_empty());
}

#[test]
fn whole_ladder_rolls_back_on_late_failure() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let conn = Connection::open(&path).unwrap();
    // v27 cannot create its table, after preceding 26 rungs have run.
    conn.execute_batch("CREATE TABLE prefix_version_restore_journal(id);")
        .unwrap();
    drop(conn);
    assert!(MetadataStore::with_path(&path).is_err());
    assert_eq!(schema(&path), 0);
    let conn = raw_open(&path, true).unwrap();
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(names, vec!["prefix_version_restore_journal"]);
    assert_eq!(
        conn.pragma_query_value(None, "application_id", |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn live_writable_handle_rejects_upgrade_for_all_helper_kinds() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let store = MetadataStore::with_path(&path).unwrap();
    let other = Connection::open(&path).unwrap();
    other.pragma_update(None, "user_version", 28).unwrap();
    assert!(matches!(
        store.dismiss_readiness_nag("test", 1),
        Err(MetadataStoreError::ReadOnlyNewerSchema)
    ));
    assert!(matches!(
        store.with_conn("write", |conn| {
            conn.execute("CREATE TABLE forbidden(id)", [])?;
            Ok(())
        }),
        Err(MetadataStoreError::ReadOnlyNewerSchema)
    ));
    assert_eq!(store.status(), newer_status(28));
    assert!(store.get_dismissed_readiness_nags().unwrap().is_empty());
}

#[test]
fn symlink_database_is_rejected() {
    let dir = tempdir().unwrap();
    let real = dir.path().join("real.db");
    drop(seed(&real, 28));
    let link = dir.path().join("metadata.db");
    symlink(&real, &link).unwrap();
    assert!(matches!(
        MetadataStore::with_path(&link),
        Err(MetadataStoreError::SymlinkDetected(_))
    ));
}

#[test]
fn fresh_database_and_new_parent_are_private_before_migrations() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("new-parent/metadata.db");
    let opened = open_with_hooks(
        &path,
        |_, _| Ok(()),
        |path| {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(opened.status, MetadataStatus::Ok);
}

#[test]
fn older_file_is_private_after_lock_before_migrations() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(seed(&path, 26));
    fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();
    open_with_hooks(
        &path,
        |_, _| Ok(()),
        |path| {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(schema(path), 26);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn older_live_wal_migrates_with_wal_and_backup_has_live_rows() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let writer = seed(&path, 26);
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    writer
        .execute_batch("CREATE TABLE live_data(value); INSERT INTO live_data VALUES ('live');")
        .unwrap();
    let store = MetadataStore::with_path(&path).unwrap();
    assert_eq!(store.status(), MetadataStatus::Ok);
    let mode: String = store
        .with_sqlite_conn("mode", |conn| {
            Ok(conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?)
        })
        .unwrap();
    assert_eq!(mode, "wal");
    assert_eq!(schema(&path), 27);
    let backup = raw_open(&store.backup_files()[0], true).unwrap();
    let value: String = backup
        .query_row("SELECT value FROM live_data", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "live");
}

#[test]
fn failed_commit_rolls_back_and_store_remains_usable() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let store = MetadataStore::with_path(&path).unwrap();
    store.with_conn_mut("setup", |conn| {
        conn.execute_batch("CREATE TABLE parent(id PRIMARY KEY); CREATE TABLE child(parent_id REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);")?; Ok(())
    }).unwrap();
    assert!(store
        .with_conn_mut("bad", |conn| {
            conn.execute("INSERT INTO child VALUES (1)", [])?;
            Ok(())
        })
        .is_err());
    store
        .with_conn_mut("good", |conn| {
            conn.execute("INSERT INTO parent VALUES (1)", [])?;
            Ok(())
        })
        .unwrap();
    let rows: i64 = store
        .with_sqlite_conn("read", |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM child", [], |row| row.get(0))?)
        })
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn panic_rolls_back_and_releases_sqlite_writer_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let store = MetadataStore::with_path(&path).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), _> = store.with_conn_mut("panic", |conn| {
            conn.execute_batch("CREATE TABLE panic_write(id);")?;
            panic!("intentional")
        });
    }));
    assert!(panic.is_err());
    let other = Connection::open(&path).unwrap();
    other
        .execute_batch("BEGIN IMMEDIATE; CREATE TABLE external_write(id); COMMIT;")
        .unwrap();
    let exists: i64 = other
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name='panic_write'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(exists, 0);
    assert!(matches!(
        store.dismiss_readiness_nag("test", 1),
        Err(MetadataStoreError::Corrupt(_))
    ));
}

#[test]
fn disabled_store_journal_operations_fail_closed_with_typed_errors() {
    let store = MetadataStore::disabled();
    let row = crate::metadata::PrefixVersionRestoreJournalRow {
        prefix_path: "/prefix".into(),
        profile_id: None,
        binary_path: "/bin/tool".into(),
        tool_type: "winetricks".into(),
        steam_app_id: None,
        restore_verb: "win10".into(),
        state: "pending".into(),
        last_error: None,
        created_at: "now".into(),
        updated_at: "now".into(),
    };
    assert!(matches!(
        store.load_prefix_version_restore("/prefix"),
        Err(MetadataStoreError::ReadOnlyDisabled { .. })
    ));
    assert!(matches!(
        store.upsert_prefix_version_restore(&row),
        Err(MetadataStoreError::ReadOnlyDisabled { .. })
    ));
    assert!(matches!(
        store.mark_prefix_version_restore_failed("/prefix", "error"),
        Err(MetadataStoreError::ReadOnlyDisabled { .. })
    ));
    assert!(matches!(
        store.delete_prefix_version_restore("/prefix"),
        Err(MetadataStoreError::ReadOnlyDisabled { .. })
    ));
}

#[test]
fn generic_sqlite_readonly_is_not_labelled_newer_schema() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let store = MetadataStore::with_path(&path).unwrap();
    let result = store.with_sqlite_conn("generic readonly", |conn| {
        conn.pragma_update(None, "query_only", true)?;
        conn.execute_batch("CREATE TABLE denied(id);")?;
        Ok(())
    });
    assert!(matches!(result, Err(MetadataStoreError::SQLiteReadonly)));
}
