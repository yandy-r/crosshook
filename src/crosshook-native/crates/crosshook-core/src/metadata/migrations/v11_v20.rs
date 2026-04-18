use super::MetadataStoreError;
use rusqlite::Connection;

pub(super) fn migrate_10_to_11(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS config_revisions (
            id                   INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id           TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            profile_name_at_write TEXT NOT NULL,
            source               TEXT NOT NULL,
            content_hash         TEXT NOT NULL,
            snapshot_toml        TEXT NOT NULL,
            source_revision_id   INTEGER REFERENCES config_revisions(id),
            is_last_known_working INTEGER NOT NULL DEFAULT 0,
            created_at           TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_config_revisions_profile_id_id
            ON config_revisions(profile_id, id DESC);
        CREATE INDEX IF NOT EXISTS idx_config_revisions_profile_id_created_at
            ON config_revisions(profile_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_config_revisions_profile_id_content_hash
            ON config_revisions(profile_id, content_hash);

        CREATE TRIGGER IF NOT EXISTS trg_config_revisions_lineage_ownership
        BEFORE INSERT ON config_revisions
        WHEN NEW.source_revision_id IS NOT NULL
        BEGIN
            SELECT RAISE(ABORT, 'source_revision_id references a revision from a different profile')
            WHERE NOT EXISTS (
                SELECT 1 FROM config_revisions
                WHERE id = NEW.source_revision_id AND profile_id = NEW.profile_id
            );
        END;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 10 to 11",
        source,
    })?;

    Ok(())
}

pub(super) fn migrate_11_to_12(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS optimization_catalog (
            sort_order              INTEGER NOT NULL,
            id                      TEXT PRIMARY KEY,
            applies_to_method       TEXT NOT NULL DEFAULT 'proton_run',
            env_json                TEXT NOT NULL DEFAULT '[]',
            wrappers_json           TEXT NOT NULL DEFAULT '[]',
            conflicts_with_json     TEXT NOT NULL DEFAULT '[]',
            required_binary         TEXT NOT NULL DEFAULT '',
            label                   TEXT NOT NULL DEFAULT '',
            description             TEXT NOT NULL DEFAULT '',
            help_text               TEXT NOT NULL DEFAULT '',
            category                TEXT NOT NULL DEFAULT '',
            target_gpu_vendor       TEXT NOT NULL DEFAULT '',
            advanced                INTEGER NOT NULL DEFAULT 0,
            community               INTEGER NOT NULL DEFAULT 0,
            applicable_methods_json TEXT NOT NULL DEFAULT '[]',
            source                  TEXT NOT NULL DEFAULT 'default',
            catalog_version         INTEGER NOT NULL DEFAULT 1,
            updated_at              TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_optimization_catalog_sort_order
            ON optimization_catalog(sort_order ASC);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 11 to 12",
        source,
    })?;

    Ok(())
}

pub(super) fn migrate_12_to_13(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS trainer_hash_cache (
            cache_id            TEXT PRIMARY KEY,
            profile_id          TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            file_path           TEXT NOT NULL,
            file_size           INTEGER,
            file_modified_at    TEXT,
            sha256_hash         TEXT NOT NULL,
            verified_at         TEXT NOT NULL,
            created_at          TEXT NOT NULL,
            updated_at          TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_trainer_hash_cache_profile_path
            ON trainer_hash_cache(profile_id, file_path);

        CREATE TABLE IF NOT EXISTS offline_readiness_snapshots (
            profile_id              TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
            readiness_state         TEXT NOT NULL DEFAULT 'unconfigured',
            readiness_score         INTEGER NOT NULL,
            trainer_type            TEXT NOT NULL DEFAULT 'unknown',
            trainer_present           INTEGER NOT NULL DEFAULT 0,
            trainer_hash_valid        INTEGER NOT NULL DEFAULT 0,
            trainer_activated         INTEGER NOT NULL DEFAULT 0,
            proton_available          INTEGER NOT NULL DEFAULT 0,
            community_tap_cached      INTEGER NOT NULL DEFAULT 0,
            network_required          INTEGER NOT NULL DEFAULT 0,
            blocking_reasons          TEXT,
            checked_at                TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS community_tap_offline_state (
            tap_id              TEXT PRIMARY KEY REFERENCES community_taps(tap_id) ON DELETE CASCADE,
            has_local_clone     INTEGER NOT NULL DEFAULT 0,
            last_sync_at        TEXT,
            clone_size_bytes    INTEGER
        );

        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 12 to 13",
        source,
    })?;

    Ok(())
}

pub(super) fn migrate_13_to_14(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS game_image_cache (
            cache_id         TEXT PRIMARY KEY,
            steam_app_id     TEXT NOT NULL,
            image_type       TEXT NOT NULL DEFAULT 'cover',
            source           TEXT NOT NULL DEFAULT 'steam_cdn',
            file_path        TEXT NOT NULL,
            file_size        INTEGER NOT NULL DEFAULT 0,
            content_hash     TEXT NOT NULL DEFAULT '',
            mime_type        TEXT NOT NULL DEFAULT 'image/jpeg',
            width            INTEGER,
            height           INTEGER,
            source_url       TEXT NOT NULL DEFAULT '',
            preferred_source TEXT NOT NULL DEFAULT 'auto',
            expires_at       TEXT,
            fetched_at       TEXT NOT NULL,
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_game_image_cache_app_type_source
            ON game_image_cache(steam_app_id, image_type, source);
        CREATE INDEX IF NOT EXISTS idx_game_image_cache_expires
            ON game_image_cache(expires_at);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 13 to 14",
        source,
    })?;

    Ok(())
}

pub(super) fn migrate_14_to_15(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS prefix_dependency_state (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            package_name     TEXT NOT NULL,
            prefix_path      TEXT NOT NULL,
            state            TEXT NOT NULL DEFAULT 'unknown',
            checked_at       TEXT,
            installed_at     TEXT,
            last_error       TEXT,
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_package_prefix
            ON prefix_dependency_state(profile_id, package_name, prefix_path);

        CREATE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_id
            ON prefix_dependency_state(profile_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "create prefix_dependency_state table (migration 14→15)",
        source,
    })
}

pub(super) fn migrate_15_to_16(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS prefix_storage_snapshots (
            id                      TEXT PRIMARY KEY,
            resolved_prefix_path    TEXT NOT NULL,
            total_bytes             INTEGER NOT NULL,
            staged_trainers_bytes   INTEGER NOT NULL,
            is_orphan               INTEGER NOT NULL,
            referenced_profiles_json TEXT NOT NULL,
            stale_staged_count      INTEGER NOT NULL,
            scanned_at              TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_prefix_storage_snapshots_prefix_path_scanned_at
            ON prefix_storage_snapshots(resolved_prefix_path, scanned_at DESC);
        CREATE INDEX IF NOT EXISTS idx_prefix_storage_snapshots_scanned_at
            ON prefix_storage_snapshots(scanned_at DESC);

        CREATE TABLE IF NOT EXISTS prefix_storage_cleanup_audit (
            id                      TEXT PRIMARY KEY,
            target_kind             TEXT NOT NULL,
            resolved_prefix_path    TEXT NOT NULL,
            target_path             TEXT NOT NULL,
            result                  TEXT NOT NULL,
            reason                  TEXT,
            reclaimed_bytes         INTEGER NOT NULL,
            created_at              TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_prefix_storage_cleanup_audit_created_at
            ON prefix_storage_cleanup_audit(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_prefix_storage_cleanup_audit_prefix_path
            ON prefix_storage_cleanup_audit(resolved_prefix_path);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "create prefix storage persistence tables (migration 15→16)",
        source,
    })
}

pub(super) fn migrate_16_to_17(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS suggestion_dismissals (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id     TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            app_id         TEXT NOT NULL,
            suggestion_key TEXT NOT NULL,
            dismissed_at   TEXT NOT NULL,
            expires_at     TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_suggestion_dismissals_unique
            ON suggestion_dismissals(profile_id, app_id, suggestion_key);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 16 to 17",
        source,
    })?;

    Ok(())
}

pub(super) fn migrate_17_to_18(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS trainer_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tap_id TEXT NOT NULL REFERENCES community_taps(tap_id) ON DELETE CASCADE,
            game_name TEXT NOT NULL,
            steam_app_id INTEGER,
            source_name TEXT NOT NULL,
            source_url TEXT NOT NULL,
            trainer_version TEXT,
            game_version TEXT,
            notes TEXT,
            sha256 TEXT,
            relative_path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(tap_id, relative_path, source_url)
        );
        CREATE INDEX IF NOT EXISTS idx_trainer_sources_game ON trainer_sources(game_name);
        CREATE INDEX IF NOT EXISTS idx_trainer_sources_app_id ON trainer_sources(steam_app_id);
        UPDATE community_taps SET last_head_commit = NULL;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 17 to 18",
        source,
    })?;

    Ok(())
}

pub(super) fn migrate_18_to_19(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        BEGIN TRANSACTION;

        -- 1. Add sort_order column to collections (NOT NULL DEFAULT 0).
        ALTER TABLE collections ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

        -- 2. Rebuild collection_profiles with ON DELETE CASCADE on profile_id.
        CREATE TABLE collection_profiles_new (
            collection_id   TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
            profile_id      TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            added_at        TEXT NOT NULL,
            PRIMARY KEY (collection_id, profile_id)
        );
        INSERT INTO collection_profiles_new (collection_id, profile_id, added_at)
        SELECT collection_id, profile_id, added_at FROM collection_profiles;
        DROP TABLE collection_profiles;
        ALTER TABLE collection_profiles_new RENAME TO collection_profiles;
        CREATE INDEX IF NOT EXISTS idx_collection_profiles_profile_id
            ON collection_profiles(profile_id);
        COMMIT;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 18 to 19",
        source,
    })?;

    Ok(())
}

/// Phase 3: per-collection launch defaults.
///
/// Adds a nullable `defaults_json TEXT` column to `collections` for storing inline JSON
/// (`CollectionDefaultsSection`). Existing rows backfill to `NULL`. Additive,
/// non-destructive — no transaction needed for a single ALTER TABLE.
pub(super) fn migrate_19_to_20(conn: &Connection) -> Result<(), MetadataStoreError> {
    let has_defaults_json = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(collections)")
            .map_err(|source| MetadataStoreError::Database {
                action: "check collections columns for defaults_json in migration 19 to 20",
                source,
            })?;
        let column_names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|source| MetadataStoreError::Database {
                action: "read collections columns for migration 19 to 20",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| MetadataStoreError::Database {
                action: "read collections columns for migration 19 to 20",
                source,
            })?;
        column_names.iter().any(|name| name == "defaults_json")
    };

    if !has_defaults_json {
        conn.execute_batch("ALTER TABLE collections ADD COLUMN defaults_json TEXT;")
            .map_err(|source| MetadataStoreError::Database {
                action: "run metadata migration 19 to 20",
                source,
            })?;
    }

    Ok(())
}
