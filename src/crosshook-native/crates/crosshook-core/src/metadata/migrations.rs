use super::MetadataStoreError;
use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), MetadataStoreError> {
    let version = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "read metadata schema version",
            source,
        })?;

    if version < 1 {
        migrate_0_to_1(conn)?;
        conn.pragma_update(None, "user_version", 1_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 2 {
        migrate_1_to_2(conn)?;
        conn.pragma_update(None, "user_version", 2_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 3 {
        migrate_2_to_3(conn)?;
        conn.pragma_update(None, "user_version", 3_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    Ok(())
}

fn migrate_0_to_1(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS profiles (
            profile_id TEXT PRIMARY KEY,
            current_filename TEXT NOT NULL UNIQUE,
            current_path TEXT NOT NULL,
            game_name TEXT,
            launch_method TEXT,
            content_hash TEXT,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            source_profile_id TEXT REFERENCES profiles(profile_id),
            deleted_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_profiles_current_filename ON profiles(current_filename);

        CREATE TABLE IF NOT EXISTS profile_name_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            old_name TEXT,
            new_name TEXT NOT NULL,
            old_path TEXT,
            new_path TEXT NOT NULL,
            source TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_profile_name_history_profile_id ON profile_name_history(profile_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 0 to 1",
        source,
    })?;

    Ok(())
}

fn migrate_1_to_2(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "ALTER TABLE profiles ADD COLUMN source TEXT;
         UPDATE profiles SET source = 'initial_census' WHERE source IS NULL;",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 1 to 2",
        source,
    })?;

    Ok(())
}

fn migrate_2_to_3(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS launchers (
            launcher_id         TEXT PRIMARY KEY,
            profile_id          TEXT REFERENCES profiles(profile_id),
            launcher_slug       TEXT NOT NULL UNIQUE,
            display_name        TEXT NOT NULL,
            script_path         TEXT NOT NULL,
            desktop_entry_path  TEXT NOT NULL,
            drift_state         TEXT NOT NULL DEFAULT 'unknown',
            created_at          TEXT NOT NULL,
            updated_at          TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_launchers_profile_id    ON launchers(profile_id);
        CREATE INDEX IF NOT EXISTS idx_launchers_launcher_slug ON launchers(launcher_slug);

        CREATE TABLE IF NOT EXISTS launch_operations (
            operation_id    TEXT PRIMARY KEY,
            profile_id      TEXT REFERENCES profiles(profile_id),
            profile_name    TEXT,
            launch_method   TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'started',
            exit_code       INTEGER,
            signal          INTEGER,
            log_path        TEXT,
            diagnostic_json TEXT,
            severity        TEXT,
            failure_mode    TEXT,
            started_at      TEXT NOT NULL,
            finished_at     TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_launch_ops_profile_id ON launch_operations(profile_id);
        CREATE INDEX IF NOT EXISTS idx_launch_ops_started_at ON launch_operations(started_at);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 2 to 3",
        source,
    })?;

    Ok(())
}
