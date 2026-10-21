use rusqlite::Connection;

use crate::metadata::db;
use crate::metadata::test_support::insert_test_profile_row;

use super::super::run_migrations;

fn migrated_conn() -> Connection {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn
}

#[test]
fn fresh_db_reaches_user_version_28_with_hmac_signature_column() {
    let conn = migrated_conn();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 28, "fresh DB should migrate to schema v28");
    assert!(conn
        .prepare("SELECT hmac_signature FROM config_revisions")
        .is_ok());
}

#[test]
fn migration_27_to_28_adds_nullable_column_and_preserves_rows() {
    let conn = migrated_conn();
    // Rebuild the v27 steady state with an existing revision row.
    conn.execute_batch("ALTER TABLE config_revisions DROP COLUMN hmac_signature;")
        .unwrap();
    conn.pragma_update(None, "user_version", 27_u32).unwrap();
    insert_test_profile_row(&conn, "profile-1");
    conn.execute(
        "INSERT INTO config_revisions
             (profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
              source_revision_id, is_last_known_working, created_at)
         VALUES ('profile-1', 'Profile', 'manual_save', 'hash-1', 'toml = 1', NULL, 1,
                 '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let row: (String, String, bool, Option<String>) = conn
        .query_row(
            "SELECT content_hash, snapshot_toml, is_last_known_working, hmac_signature
             FROM config_revisions WHERE profile_id = 'profile-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        ("hash-1".to_string(), "toml = 1".to_string(), true, None),
        "existing revisions must be untouched with a NULL signature"
    );
}
