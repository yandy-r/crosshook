use rusqlite::{params, Connection};

use crate::metadata::db;
use crate::metadata::test_support::insert_test_profile_row;

use super::super::run_migrations;

fn migrated_conn() -> Connection {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn
}

#[test]
fn fresh_db_reaches_user_version_27() {
    let conn = migrated_conn();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 27, "fresh DB should migrate to schema v27");
}

#[test]
fn prefix_version_restore_journal_enforces_one_row_per_prefix() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    conn.execute(
        "INSERT INTO prefix_version_restore_journal
         (prefix_path, profile_id, binary_path, tool_type, steam_app_id,
          restore_verb, state, last_error, created_at, updated_at)
         VALUES (?1, ?2, 'winetricks', 'winetricks', NULL, 'win10', 'pending',
                 NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        params!["/games/pfx", "profile-1"],
    )
    .unwrap();

    let duplicate = conn.execute(
        "INSERT INTO prefix_version_restore_journal
         (prefix_path, profile_id, binary_path, tool_type, steam_app_id,
          restore_verb, state, last_error, created_at, updated_at)
         VALUES (?1, ?2, 'winetricks', 'winetricks', NULL, 'win11', 'pending',
                 NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        params!["/games/pfx", "profile-1"],
    );
    assert!(duplicate.is_err());
}

#[test]
fn prefix_version_restore_journal_cascades_on_profile_delete() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    conn.execute(
        "INSERT INTO prefix_version_restore_journal
         (prefix_path, profile_id, binary_path, tool_type, restore_verb, state,
          created_at, updated_at)
         VALUES ('/games/pfx', 'profile-1', 'winetricks', 'winetricks', 'win10',
                 'pending', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute("DELETE FROM profiles WHERE profile_id = 'profile-1'", [])
        .unwrap();
    let remaining: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM prefix_version_restore_journal",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}
