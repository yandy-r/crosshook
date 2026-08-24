use rusqlite::{params, Connection};

use crate::metadata::db;
use crate::metadata::test_support::insert_test_profile_row;

use super::super::run_migrations;

fn migrated_conn() -> Connection {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn
}

fn insert_mod_row(
    conn: &Connection,
    mod_id: &str,
    profile_id: &str,
    name: &str,
    category: &str,
    provenance: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO profile_mods (
            mod_id, profile_id, name, category, paths_json, enabled, provenance,
            source_url, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, '[\"dxgi.dll\"]', 1, ?5,
                  'https://example.com/mod', '2026-01-01T00:00:00+00:00',
                  '2026-01-02T00:00:00+00:00')",
        params![mod_id, profile_id, name, category, provenance],
    )
}

#[test]
fn fresh_db_reaches_current_user_version() {
    let conn = migrated_conn();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 27, "fresh DB should migrate to current schema v27");
}

#[test]
fn profile_mods_table_shape() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    insert_mod_row(
        &conn,
        "mod-1",
        "profile-1",
        "ReShade",
        "overlay_injection",
        "detected",
    )
    .unwrap();

    let row = conn
        .query_row(
            "SELECT mod_id, profile_id, name, category, paths_json, enabled, provenance,
                    source_url, created_at, updated_at
             FROM profile_mods WHERE mod_id = 'mod-1'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(
        row,
        (
            "mod-1".to_string(),
            "profile-1".to_string(),
            "ReShade".to_string(),
            "overlay_injection".to_string(),
            "[\"dxgi.dll\"]".to_string(),
            true,
            "detected".to_string(),
            Some("https://example.com/mod".to_string()),
            "2026-01-01T00:00:00+00:00".to_string(),
            "2026-01-02T00:00:00+00:00".to_string(),
        )
    );
}

#[test]
fn profile_mods_rejects_invalid_category() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    let err = insert_mod_row(&conn, "mod-1", "profile-1", "Bogus", "bogus", "manual");
    assert!(
        err.is_err(),
        "CHECK constraint should reject category='bogus'"
    );
}

#[test]
fn profile_mods_rejects_invalid_provenance() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    let err = insert_mod_row(&conn, "mod-1", "profile-1", "Bogus", "other", "auto");
    assert!(
        err.is_err(),
        "CHECK constraint should reject provenance='auto'"
    );
}

#[test]
fn profile_mods_unique_profile_name() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    insert_mod_row(
        &conn,
        "mod-1",
        "profile-1",
        "SKSE",
        "script_extender",
        "manual",
    )
    .unwrap();
    let dup = insert_mod_row(
        &conn,
        "mod-2",
        "profile-1",
        "SKSE",
        "script_extender",
        "manual",
    );
    assert!(
        dup.is_err(),
        "duplicate (profile_id, name) should violate UNIQUE"
    );
}

#[test]
fn profile_mods_unique_profile_name_is_case_insensitive() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    insert_mod_row(
        &conn,
        "mod-1",
        "profile-1",
        "ReShade",
        "overlay_injection",
        "manual",
    )
    .unwrap();
    let dup = insert_mod_row(
        &conn,
        "mod-2",
        "profile-1",
        "reshade",
        "overlay_injection",
        "manual",
    );
    assert!(
        dup.is_err(),
        "(profile_id, name) uniqueness must be case-insensitive"
    );
}

#[test]
fn hard_delete_profile_cascades_mods() {
    let conn = migrated_conn();
    insert_test_profile_row(&conn, "profile-1");
    insert_mod_row(
        &conn,
        "mod-1",
        "profile-1",
        "SKSE",
        "script_extender",
        "manual",
    )
    .unwrap();

    conn.execute("DELETE FROM profiles WHERE profile_id = 'profile-1'", [])
        .unwrap();

    let remaining: u32 = conn
        .query_row("SELECT COUNT(*) FROM profile_mods", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        remaining, 0,
        "profile delete should cascade to profile_mods"
    );
}
