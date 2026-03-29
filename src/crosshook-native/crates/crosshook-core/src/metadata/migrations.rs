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

    if version < 4 {
        migrate_3_to_4(conn)?;
        conn.pragma_update(None, "user_version", 4_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 5 {
        migrate_4_to_5(conn)?;
        conn.pragma_update(None, "user_version", 5_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 6 {
        migrate_5_to_6(conn)?;
        conn.pragma_update(None, "user_version", 6_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 7 {
        migrate_6_to_7(conn)?;
        conn.pragma_update(None, "user_version", 7_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 8 {
        migrate_7_to_8(conn)?;
        conn.pragma_update(None, "user_version", 8_u32)
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

fn migrate_3_to_4(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS community_taps (
            tap_id              TEXT PRIMARY KEY,
            tap_url             TEXT NOT NULL,
            tap_branch          TEXT NOT NULL DEFAULT '',
            local_path          TEXT NOT NULL,
            last_head_commit    TEXT,
            profile_count       INTEGER NOT NULL DEFAULT 0,
            last_indexed_at     TEXT,
            created_at          TEXT NOT NULL,
            updated_at          TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_community_taps_url_branch ON community_taps(tap_url, tap_branch);

        CREATE TABLE IF NOT EXISTS community_profiles (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            tap_id              TEXT NOT NULL REFERENCES community_taps(tap_id) ON DELETE CASCADE,
            relative_path       TEXT NOT NULL,
            manifest_path       TEXT NOT NULL,
            game_name           TEXT,
            game_version        TEXT,
            trainer_name        TEXT,
            trainer_version     TEXT,
            proton_version      TEXT,
            compatibility_rating TEXT,
            author              TEXT,
            description         TEXT,
            platform_tags       TEXT,
            schema_version      INTEGER NOT NULL DEFAULT 1,
            created_at          TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_community_profiles_tap_path ON community_profiles(tap_id, relative_path);

        CREATE TABLE IF NOT EXISTS external_cache_entries (
            cache_id        TEXT PRIMARY KEY,
            source_url      TEXT NOT NULL,
            cache_key       TEXT NOT NULL UNIQUE,
            payload_json    TEXT,
            payload_size    INTEGER NOT NULL DEFAULT 0,
            fetched_at      TEXT NOT NULL,
            expires_at      TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS collections (
            collection_id   TEXT PRIMARY KEY,
            name            TEXT NOT NULL UNIQUE,
            description     TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS collection_profiles (
            collection_id   TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
            profile_id      TEXT NOT NULL REFERENCES profiles(profile_id),
            added_at        TEXT NOT NULL,
            PRIMARY KEY (collection_id, profile_id)
        );
        CREATE INDEX IF NOT EXISTS idx_collection_profiles_profile_id ON collection_profiles(profile_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 3 to 4",
        source,
    })?;

    Ok(())
}

fn migrate_4_to_5(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        BEGIN TRANSACTION;
        ALTER TABLE community_profiles RENAME TO community_profiles_old;

        CREATE TABLE community_profiles (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            tap_id              TEXT NOT NULL REFERENCES community_taps(tap_id) ON DELETE CASCADE,
            relative_path       TEXT NOT NULL,
            manifest_path       TEXT NOT NULL,
            game_name           TEXT,
            game_version        TEXT,
            trainer_name        TEXT,
            trainer_version     TEXT,
            proton_version      TEXT,
            compatibility_rating TEXT,
            author              TEXT,
            description         TEXT,
            platform_tags       TEXT,
            schema_version      INTEGER NOT NULL DEFAULT 1,
            created_at          TEXT NOT NULL
        );

        INSERT INTO community_profiles (
            id, tap_id, relative_path, manifest_path,
            game_name, game_version, trainer_name, trainer_version,
            proton_version, compatibility_rating, author, description,
            platform_tags, schema_version, created_at
        )
        SELECT
            id, tap_id, relative_path, manifest_path,
            game_name, game_version, trainer_name, trainer_version,
            proton_version, compatibility_rating, author, description,
            platform_tags, schema_version, created_at
        FROM community_profiles_old;

        DROP TABLE community_profiles_old;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_community_profiles_tap_path
            ON community_profiles(tap_id, relative_path);
        COMMIT;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 4 to 5",
        source,
    })?;

    Ok(())
}

fn migrate_5_to_6(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS health_snapshots (
            profile_id  TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
            status      TEXT NOT NULL,
            issue_count INTEGER NOT NULL DEFAULT 0,
            checked_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_health_snapshots_checked_at ON health_snapshots(checked_at);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 5 to 6",
        source,
    })?;

    Ok(())
}

fn migrate_6_to_7(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        BEGIN TRANSACTION;
        CREATE TABLE health_snapshots_new (
            profile_id  TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
            status      TEXT NOT NULL,
            issue_count INTEGER NOT NULL DEFAULT 0,
            checked_at  TEXT NOT NULL
        );
        INSERT INTO health_snapshots_new (profile_id, status, issue_count, checked_at)
        SELECT profile_id, status, issue_count, checked_at
        FROM health_snapshots;
        DROP TABLE health_snapshots;
        ALTER TABLE health_snapshots_new RENAME TO health_snapshots;
        CREATE INDEX IF NOT EXISTS idx_health_snapshots_checked_at ON health_snapshots(checked_at);
        COMMIT;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 6 to 7",
        source,
    })?;

    Ok(())
}

fn migrate_7_to_8(conn: &Connection) -> Result<(), MetadataStoreError> {
    let has_column = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(profiles)")
            .map_err(|source| MetadataStoreError::Database {
                action: "check profiles columns for migration 7 to 8",
                source,
            })?;
        let found = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|source| MetadataStoreError::Database {
                action: "read profiles columns for migration 7 to 8",
                source,
            })?
            .any(|name| matches!(name.as_deref(), Ok("is_pinned")));
        found
    };

    if has_column {
        conn.execute_batch("ALTER TABLE profiles DROP COLUMN is_pinned;")
            .map_err(|source| MetadataStoreError::Database {
                action: "run metadata migration 7 to 8",
                source,
            })?;
    }

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
