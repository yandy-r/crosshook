use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use crate::metadata::hash_trainer_file;
use crate::metadata::offline_store::{lookup_trainer_hash_cache, upsert_trainer_hash_cache};
use crate::metadata::MetadataStoreError;
use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HashVerifyResult {
    pub hash: String,
    pub from_cache: bool,
    pub file_size: u64,
}

fn file_mtime_rfc3339(meta: &std::fs::Metadata) -> Option<String> {
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(UNIX_EPOCH).ok()?;
    Utc.timestamp_opt(dur.as_secs() as i64, dur.subsec_nanos())
        .single()
        .map(|dt| dt.to_rfc3339())
}

/// Stat, compare to SQLite cache, re-hash when stale; upsert and return digest.
pub fn verify_and_cache_trainer_hash(
    conn: &Connection,
    profile_id: &str,
    trainer_path: &Path,
) -> Result<Option<HashVerifyResult>, MetadataStoreError> {
    let meta = match std::fs::metadata(trainer_path) {
        Ok(m) if m.is_file() => m,
        _ => return Ok(None),
    };

    let file_size = meta.len();
    let file_path = trainer_path.to_string_lossy().into_owned();
    let file_modified_at = file_mtime_rfc3339(&meta);
    let size_i64 = i64::try_from(file_size).map_err(|_| {
        MetadataStoreError::Validation("trainer file size exceeds i64 range".to_string())
    })?;

    if let Some(row) = lookup_trainer_hash_cache(conn, profile_id, &file_path)? {
        if row.file_size == Some(size_i64)
            && file_modified_at.as_deref() == row.file_modified_at.as_deref()
        {
            return Ok(Some(HashVerifyResult {
                hash: row.sha256_hash,
                from_cache: true,
                file_size,
            }));
        }
    }

    let Some(hash) = hash_trainer_file(trainer_path) else {
        return Ok(None);
    };

    let now = Utc::now().to_rfc3339();
    let cache_id = Uuid::new_v4().simple().to_string();
    upsert_trainer_hash_cache(
        conn,
        &cache_id,
        profile_id,
        &file_path,
        Some(size_i64),
        file_modified_at.as_deref(),
        &hash,
        &now,
        &now,
        &now,
    )?;

    Ok(Some(HashVerifyResult {
        hash,
        from_cache: false,
        file_size,
    }))
}

