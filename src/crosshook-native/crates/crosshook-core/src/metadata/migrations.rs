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

    if version < 9 {
        migrate_8_to_9(conn)?;
        conn.pragma_update(None, "user_version", 9_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 10 {
        migrate_9_to_10(conn)?;
        conn.pragma_update(None, "user_version", 10_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 11 {
        migrate_10_to_11(conn)?;
        conn.pragma_update(None, "user_version", 11_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 12 {
        migrate_11_to_12(conn)?;
        conn.pragma_update(None, "user_version", 12_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 12",
                source,
            })?;
    }

    if version < 13 {
        migrate_12_to_13(conn)?;
        conn.pragma_update(None, "user_version", 13_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 13",
                source,
            })?;
    }

    if version < 14 {
        migrate_13_to_14(conn)?;
        conn.pragma_update(None, "user_version", 14_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 14",
                source,
            })?;
    }

    if version < 15 {
        migrate_14_to_15(conn)?;
        conn.pragma_update(None, "user_version", 15_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 15",
                source,
            })?;
    }

    if version < 16 {
        migrate_15_to_16(conn)?;
        conn.pragma_update(None, "user_version", 16_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 16",
                source,
            })?;
    }

    if version < 17 {
        migrate_16_to_17(conn)?;
        conn.pragma_update(None, "user_version", 17_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 17",
                source,
            })?;
    }

    if version < 18 {
        migrate_17_to_18(conn)?;
        conn.pragma_update(None, "user_version", 18_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 18",
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

fn migrate_9_to_10(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bundled_optimization_presets (
            preset_id           TEXT PRIMARY KEY,
            display_name        TEXT NOT NULL,
            vendor              TEXT NOT NULL,
            mode                TEXT NOT NULL,
            option_ids_json     TEXT NOT NULL,
            catalog_version     INTEGER NOT NULL DEFAULT 1,
            created_at          TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_bundled_optimization_presets_vendor_mode
            ON bundled_optimization_presets(vendor, mode);

        CREATE TABLE IF NOT EXISTS profile_launch_preset_metadata (
            profile_id                  TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            preset_name                 TEXT NOT NULL,
            origin                      TEXT NOT NULL,
            source_bundled_preset_id    TEXT,
            created_at                  TEXT NOT NULL,
            updated_at                  TEXT NOT NULL,
            PRIMARY KEY (profile_id, preset_name)
        );
        CREATE INDEX IF NOT EXISTS idx_profile_launch_preset_metadata_profile_id
            ON profile_launch_preset_metadata(profile_id);

        INSERT INTO bundled_optimization_presets
            (preset_id, display_name, vendor, mode, option_ids_json, catalog_version, created_at)
        VALUES
            ('nvidia_performance', 'NVIDIA · Performance', 'nvidia', 'performance',
             '[\"use_gamemode\",\"enable_nvapi\",\"enable_nvidia_libs\",\"enable_dxvk_async\",\"use_ntsync\",\"force_large_address_aware\"]',
             1, datetime('now')),
            ('nvidia_quality', 'NVIDIA · Quality', 'nvidia', 'quality',
             '[\"enable_nvapi\",\"enable_nvidia_libs\",\"enable_dlss_upgrade\",\"show_dlss_indicator\",\"enable_hdr\",\"enable_local_shader_cache\"]',
             1, datetime('now')),
            ('amd_performance', 'AMD · Performance', 'amd', 'performance',
             '[\"use_gamemode\",\"enable_dxvk_async\",\"use_ntsync\",\"force_large_address_aware\",\"enable_fsr4_upgrade\"]',
             1, datetime('now')),
            ('amd_quality', 'AMD · Quality', 'amd', 'quality',
             '[\"enable_fsr4_upgrade\",\"enable_xess_upgrade\",\"enable_hdr\",\"enable_local_shader_cache\",\"enable_vkd3d_dxr\"]',
             1, datetime('now'));
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 9 to 10",
        source,
    })?;

    Ok(())
}

fn migrate_8_to_9(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS version_snapshots (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id          TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            steam_app_id        TEXT NOT NULL DEFAULT '',
            steam_build_id      TEXT,
            trainer_version     TEXT,
            trainer_file_hash   TEXT,
            human_game_ver      TEXT,
            status              TEXT NOT NULL DEFAULT 'untracked',
            checked_at          TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_profile_checked
            ON version_snapshots(profile_id, checked_at DESC);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_steam_app_id
            ON version_snapshots(steam_app_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 8 to 9",
        source,
    })?;

    Ok(())
}

fn migrate_10_to_11(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_12_to_13(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_11_to_12(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_14_to_15(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_13_to_14(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_15_to_16(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_17_to_18(conn: &Connection) -> Result<(), MetadataStoreError> {
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

fn migrate_16_to_17(conn: &Connection) -> Result<(), MetadataStoreError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;

    #[test]
    fn migration_11_to_12_creates_optimization_catalog_table() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // Verify the table exists by querying it
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM optimization_catalog", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn migration_12_to_13_creates_offline_tables() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        for table in [
            "trainer_hash_cache",
            "offline_readiness_snapshots",
            "community_tap_offline_state",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {table}");
        }
    }

    #[test]
    fn migration_13_to_14_creates_game_image_cache_table() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify the table exists
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_image_cache'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "missing table game_image_cache");

        // Verify the unique index exists
        let idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_game_image_cache_app_type_source'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1, "missing index idx_game_image_cache_app_type_source");

        let expires_idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_game_image_cache_expires'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(expires_idx, 1, "missing index idx_game_image_cache_expires");
    }

    #[test]
    fn migration_14_to_15_creates_prefix_dependency_state_table() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify schema version (latest after all migrations)
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert!(
            version >= 15,
            "schema version should be at least 15, got {version}"
        );

        // Verify table exists
        let table_exists: bool = conn
            .prepare(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='prefix_dependency_state'",
            )
            .unwrap()
            .exists([])
            .unwrap();
        assert!(table_exists, "prefix_dependency_state table should exist");

        // Verify unique index exists
        let idx_exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_prefix_dep_state_profile_package_prefix'")
            .unwrap()
            .exists([])
            .unwrap();
        assert!(idx_exists, "unique index should exist");

        // Verify FK cascade: insert profile, insert dep state, delete profile, verify cascade
        conn.execute(
            "INSERT INTO profiles (profile_id, current_filename, current_path, game_name, created_at, updated_at)
             VALUES ('test-prof', 'test.toml', '/tmp/test.toml', 'Test', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO prefix_dependency_state (profile_id, package_name, prefix_path, state, created_at, updated_at)
             VALUES ('test-prof', 'vcrun2019', '/tmp/pfx', 'installed', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // Delete profile
        conn.execute("DELETE FROM profiles WHERE profile_id = 'test-prof'", [])
            .unwrap();

        // Verify cascade deleted the dep state
        let dep_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prefix_dependency_state WHERE profile_id = 'test-prof'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            dep_count, 0,
            "dep state should be cascade-deleted with profile"
        );
    }

    #[test]
    fn migration_15_to_16_creates_prefix_storage_tables() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert!(
            version >= 16,
            "schema version should be at least 16, got {version}"
        );

        for table in ["prefix_storage_snapshots", "prefix_storage_cleanup_audit"] {
            let exists: bool = conn
                .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1")
                .unwrap()
                .exists([table])
                .unwrap();
            assert!(exists, "missing table {table}");
        }

        for idx in [
            "idx_prefix_storage_snapshots_prefix_path_scanned_at",
            "idx_prefix_storage_snapshots_scanned_at",
            "idx_prefix_storage_cleanup_audit_created_at",
            "idx_prefix_storage_cleanup_audit_prefix_path",
        ] {
            let exists: bool = conn
                .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1")
                .unwrap()
                .exists([idx])
                .unwrap();
            assert!(exists, "missing index {idx}");
        }
    }

    #[test]
    fn migration_16_to_17_creates_suggestion_dismissals_table() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert!(
            version >= 17,
            "schema version should be at least 17, got {version}"
        );

        let table_exists: bool = conn
            .prepare(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='suggestion_dismissals'",
            )
            .unwrap()
            .exists([])
            .unwrap();
        assert!(table_exists, "suggestion_dismissals table should exist");

        let idx_exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_suggestion_dismissals_unique'")
            .unwrap()
            .exists([])
            .unwrap();
        assert!(
            idx_exists,
            "unique index idx_suggestion_dismissals_unique should exist"
        );
    }

    #[test]
    fn migration_17_to_18_creates_trainer_sources_table() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 18);

        let table_exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='trainer_sources'")
            .unwrap()
            .exists([])
            .unwrap();
        assert!(table_exists, "trainer_sources table should exist");

        let game_idx_exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_trainer_sources_game'")
            .unwrap()
            .exists([])
            .unwrap();
        assert!(
            game_idx_exists,
            "index idx_trainer_sources_game should exist"
        );

        let app_id_idx_exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_trainer_sources_app_id'")
            .unwrap()
            .exists([])
            .unwrap();
        assert!(
            app_id_idx_exists,
            "index idx_trainer_sources_app_id should exist"
        );
    }
}
