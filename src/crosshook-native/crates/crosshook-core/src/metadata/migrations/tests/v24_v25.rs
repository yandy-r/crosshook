use rusqlite::params;

use crate::metadata::db;

use super::super::run_migrations;

#[test]
fn migration_24_to_25_adds_trainer_loading_mode_column() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 25,
        "schema version should be at least 25 after migration 24→25, got {version}"
    );

    assert!(
        conn.prepare("SELECT trainer_loading_mode FROM community_profiles")
            .is_ok(),
        "community_profiles.trainer_loading_mode column should exist"
    );

    conn.execute(
        "INSERT INTO community_taps (tap_id, tap_url, tap_branch, local_path, created_at, updated_at)
         VALUES ('tap-001', 'https://example.com/tap.git', '', '/tmp/tap', datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO community_profiles (
            tap_id, relative_path, manifest_path, trainer_loading_mode, schema_version, created_at
        ) VALUES ('tap-001', 'game/community-profile.json', '/tmp/tap/game/community-profile.json',
                  'copy_to_prefix', 1, datetime('now'))",
        [],
    )
    .unwrap();

    let stored: Option<String> = conn
        .query_row(
            "SELECT trainer_loading_mode FROM community_profiles WHERE tap_id = 'tap-001'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored.as_deref(), Some("copy_to_prefix"));
}

#[test]
fn migration_24_to_25_resets_tap_watermarks() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    // Rebuild the v24 steady state: no trainer_loading_mode column, a stored
    // watermark, and user_version pinned at 24 so run_migrations replays 24→25.
    conn.execute_batch("ALTER TABLE community_profiles DROP COLUMN trainer_loading_mode")
        .unwrap();
    conn.pragma_update(None, "user_version", 24_u32).unwrap();
    conn.execute(
        "INSERT INTO community_taps (
            tap_id, tap_url, tap_branch, local_path, last_head_commit, created_at, updated_at
        ) VALUES ('tap-001', 'https://example.com/tap.git', '', '/tmp/tap', 'abc1234',
                  datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();

    run_migrations(&conn).unwrap();

    let watermark: Option<String> = conn
        .query_row(
            "SELECT last_head_commit FROM community_taps WHERE tap_id = ?1",
            params!["tap-001"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        watermark, None,
        "migration 24→25 must reset tap watermarks so the next sync backfills the column"
    );

    assert!(
        conn.prepare("SELECT trainer_loading_mode FROM community_profiles")
            .is_ok(),
        "replaying 24→25 must add the trainer_loading_mode column back"
    );
}
