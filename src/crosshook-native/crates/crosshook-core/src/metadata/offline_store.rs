use super::MetadataStoreError;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct TrainerHashCacheRow {
    pub cache_id: String,
    pub profile_id: String,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub file_modified_at: Option<String>,
    pub sha256_hash: String,
    pub verified_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct OfflineReadinessRow {
    pub profile_id: String,
    pub profile_name: String,
    pub readiness_state: String,
    pub readiness_score: i64,
    pub trainer_type: String,
    pub trainer_present: i64,
    pub trainer_hash_valid: i64,
    pub trainer_activated: i64,
    pub proton_available: i64,
    pub community_tap_cached: i64,
    pub network_required: i64,
    pub blocking_reasons: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone)]
pub struct CommunityTapOfflineRow {
    pub tap_id: String,
    pub has_local_clone: i64,
    pub last_sync_at: Option<String>,
    pub clone_size_bytes: Option<i64>,
}

pub fn upsert_trainer_hash_cache(
    conn: &Connection,
    cache_id: &str,
    profile_id: &str,
    file_path: &str,
    file_size: Option<i64>,
    file_modified_at: Option<&str>,
    sha256_hash: &str,
    verified_at: &str,
    created_at: &str,
    updated_at: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT INTO trainer_hash_cache
            (cache_id, profile_id, file_path, file_size, file_modified_at, sha256_hash, verified_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(profile_id, file_path) DO UPDATE SET
            file_size = excluded.file_size,
            file_modified_at = excluded.file_modified_at,
            sha256_hash = excluded.sha256_hash,
            verified_at = excluded.verified_at,
            updated_at = excluded.updated_at",
        params![
            cache_id,
            profile_id,
            file_path,
            file_size,
            file_modified_at,
            sha256_hash,
            verified_at,
            created_at,
            updated_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert trainer_hash_cache",
        source,
    })?;
    Ok(())
}

pub fn lookup_trainer_hash_cache(
    conn: &Connection,
    profile_id: &str,
    file_path: &str,
) -> Result<Option<TrainerHashCacheRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT cache_id, profile_id, file_path, file_size, file_modified_at,
                    sha256_hash, verified_at, created_at, updated_at
             FROM trainer_hash_cache
             WHERE profile_id = ?1 AND file_path = ?2",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare lookup_trainer_hash_cache",
            source,
        })?;

    stmt.query_row(params![profile_id, file_path], |row| {
        Ok(TrainerHashCacheRow {
            cache_id: row.get(0)?,
            profile_id: row.get(1)?,
            file_path: row.get(2)?,
            file_size: row.get(3)?,
            file_modified_at: row.get(4)?,
            sha256_hash: row.get(5)?,
            verified_at: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "lookup trainer_hash_cache",
        source,
    })
}

pub fn upsert_offline_readiness_snapshot(
    conn: &Connection,
    profile_id: &str,
    readiness_state: &str,
    readiness_score: i64,
    trainer_type: &str,
    trainer_present: i64,
    trainer_hash_valid: i64,
    trainer_activated: i64,
    proton_available: i64,
    community_tap_cached: i64,
    network_required: i64,
    blocking_reasons: Option<&str>,
    checked_at: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT OR REPLACE INTO offline_readiness_snapshots
            (profile_id, readiness_state, readiness_score, trainer_type, trainer_present,
             trainer_hash_valid, trainer_activated, proton_available, community_tap_cached,
             network_required, blocking_reasons, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            profile_id,
            readiness_state,
            readiness_score,
            trainer_type,
            trainer_present,
            trainer_hash_valid,
            trainer_activated,
            proton_available,
            community_tap_cached,
            network_required,
            blocking_reasons,
            checked_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert offline_readiness_snapshots",
        source,
    })?;
    Ok(())
}

pub fn load_offline_readiness_snapshots(
    conn: &Connection,
) -> Result<Vec<OfflineReadinessRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT o.profile_id, p.current_filename, o.readiness_state, o.readiness_score,
                    o.trainer_type, o.trainer_present, o.trainer_hash_valid, o.trainer_activated,
                    o.proton_available, o.community_tap_cached, o.network_required,
                    o.blocking_reasons, o.checked_at
             FROM offline_readiness_snapshots o
             INNER JOIN profiles p ON o.profile_id = p.profile_id
             WHERE p.deleted_at IS NULL",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare load_offline_readiness_snapshots",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(OfflineReadinessRow {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                readiness_state: row.get(2)?,
                readiness_score: row.get(3)?,
                trainer_type: row.get(4)?,
                trainer_present: row.get(5)?,
                trainer_hash_valid: row.get(6)?,
                trainer_activated: row.get(7)?,
                proton_available: row.get(8)?,
                community_tap_cached: row.get(9)?,
                network_required: row.get(10)?,
                blocking_reasons: row.get(11)?,
                checked_at: row.get(12)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query offline_readiness_snapshots",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect offline_readiness rows",
            source,
        })?;

    Ok(rows)
}

pub fn upsert_community_tap_offline_state(
    conn: &Connection,
    tap_id: &str,
    has_local_clone: i64,
    last_sync_at: Option<&str>,
    clone_size_bytes: Option<i64>,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT INTO community_tap_offline_state (tap_id, has_local_clone, last_sync_at, clone_size_bytes)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(tap_id) DO UPDATE SET
            has_local_clone = excluded.has_local_clone,
            last_sync_at = excluded.last_sync_at,
            clone_size_bytes = excluded.clone_size_bytes",
        params![tap_id, has_local_clone, last_sync_at, clone_size_bytes],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert community_tap_offline_state",
        source,
    })?;
    Ok(())
}

pub fn lookup_community_tap_offline_state(
    conn: &Connection,
    tap_id: &str,
) -> Result<Option<CommunityTapOfflineRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT tap_id, has_local_clone, last_sync_at, clone_size_bytes
             FROM community_tap_offline_state
             WHERE tap_id = ?1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare lookup_community_tap_offline_state",
            source,
        })?;

    stmt.query_row(params![tap_id], |row| {
        Ok(CommunityTapOfflineRow {
            tap_id: row.get(0)?,
            has_local_clone: row.get(1)?,
            last_sync_at: row.get(2)?,
            clone_size_bytes: row.get(3)?,
        })
    })
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "lookup community_tap_offline_state",
        source,
    })
}
