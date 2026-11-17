use rusqlite::Connection;

use crate::metadata::db;

use super::super::run_migrations;

fn migrated_conn() -> Connection {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn
}

fn user_version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn index_exists(conn: &Connection, name: &str) -> bool {
    conn.prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1")
        .unwrap()
        .exists([name])
        .unwrap()
}

#[test]
fn fresh_db_reaches_user_version_29_without_unused_indexes() {
    let conn = migrated_conn();
    assert_eq!(
        user_version(&conn),
        29,
        "fresh DB should migrate to schema v29"
    );
    assert!(!index_exists(&conn, "idx_trainer_sources_app_id"));
    assert!(!index_exists(&conn, "idx_game_image_cache_expires"));
}

#[test]
fn migration_28_to_29_drops_unused_indexes_and_preserves_rows() {
    let conn = migrated_conn();
    // Rebuild the v28 steady state: both indexes present, with one row in each affected table.
    conn.execute_batch(
        "CREATE INDEX idx_trainer_sources_app_id ON trainer_sources(steam_app_id);
         CREATE INDEX idx_game_image_cache_expires ON game_image_cache(expires_at);",
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 28_u32).unwrap();
    conn.execute(
        "INSERT INTO game_image_cache
             (cache_id, steam_app_id, image_type, source, file_path, fetched_at, created_at, updated_at)
         VALUES ('image-1', '440', 'cover', 'steam_cdn', '/cache/440.jpg',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    run_migrations(&conn).unwrap();

    assert_eq!(user_version(&conn), 29);
    assert!(!index_exists(&conn, "idx_trainer_sources_app_id"));
    assert!(!index_exists(&conn, "idx_game_image_cache_expires"));
    let file_path: String = conn
        .query_row(
            "SELECT file_path FROM game_image_cache WHERE cache_id = 'image-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(file_path, "/cache/440.jpg");
}
