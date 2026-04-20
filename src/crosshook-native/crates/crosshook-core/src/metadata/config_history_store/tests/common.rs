use crate::metadata::{db, migrations, ConfigRevisionSource};
use rusqlite::{params, Connection};

pub(super) fn open_test_db() -> Connection {
    let conn = db::open_in_memory().expect("open in-memory db");
    migrations::run_migrations(&conn).expect("run migrations");
    conn
}

/// Insert a minimal `profiles` row so that `config_revisions` FK constraints
/// are satisfied. The `profile_id` doubles as the filename to keep tests
/// self-contained.
pub(super) fn ensure_profile(conn: &Connection, profile_id: &str) {
    let now = "2024-01-01T00:00:00Z";
    conn.execute(
        "INSERT OR IGNORE INTO profiles
             (profile_id, current_filename, current_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            profile_id,
            profile_id,
            format!("/profiles/{profile_id}.toml"),
            now,
            now,
        ],
    )
    .expect("ensure_profile insert must not fail");
}

pub(super) fn insert_revision(conn: &Connection, profile_id: &str, hash: &str) -> i64 {
    ensure_profile(conn, profile_id);
    crate::metadata::config_history_store::insert_config_revision(
        conn,
        profile_id,
        "Test Profile",
        ConfigRevisionSource::ManualSave,
        hash,
        "some toml content",
        None,
    )
    .expect("insert must not fail")
    .expect("insert must not be deduped against a different hash")
}
