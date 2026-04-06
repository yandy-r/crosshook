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

/// Normalizes a 64-character hex SHA-256 digest (optional `0x` prefix).
pub fn normalize_sha256_hex(input: &str) -> Option<String> {
    let s = input.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s)
        .trim();
    if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(s.to_ascii_lowercase())
}

/// Result of comparing the on-disk trainer digest to the cached baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrainerHashBaselineResult {
    /// Trainer path missing or not a file.
    SkippedNoTrainerPath,
    /// File exists but could not be hashed.
    Unverifiable,
    /// Cached baseline matches the current file (or metadata was refreshed without content change).
    OkMatched,
    /// No prior row: baseline was written to the cache.
    FirstBaselineRecorded,
    /// File content changed vs the stored baseline; cache is not updated (user must confirm).
    Mismatch {
        stored_hash: String,
        current_hash: String,
    },
}

impl Default for TrainerHashBaselineResult {
    fn default() -> Self {
        Self::SkippedNoTrainerPath
    }
}

/// Advisory-only comparison against a community manifest digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerHashCommunityAdvisory {
    pub expected: String,
    pub current: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrainerHashLaunchOutcome {
    pub baseline: TrainerHashBaselineResult,
    pub community_advisory: Option<TrainerHashCommunityAdvisory>,
}

/// Compares the current trainer file hash to SQLite `trainer_hash_cache` and optional community digest.
///
/// On first seen `(profile_id, file_path)`, stores the current hash as baseline. On content change
/// vs stored baseline, returns [`TrainerHashBaselineResult::Mismatch`] without overwriting the cache
/// (call [`verify_and_cache_trainer_hash`] / `verify_trainer_hash` IPC after user confirmation).
pub fn trainer_hash_launch_check(
    conn: &Connection,
    profile_id: &str,
    trainer_path: &Path,
    community_trainer_sha256: Option<&str>,
) -> Result<TrainerHashLaunchOutcome, MetadataStoreError> {
    let mut outcome = TrainerHashLaunchOutcome::default();

    let meta = match std::fs::metadata(trainer_path) {
        Ok(m) if m.is_file() => m,
        _ => {
            outcome.baseline = TrainerHashBaselineResult::SkippedNoTrainerPath;
            return Ok(outcome);
        }
    };

    let Some(current_hash) = hash_trainer_file(trainer_path) else {
        outcome.baseline = TrainerHashBaselineResult::Unverifiable;
        return Ok(outcome);
    };

    let file_path = trainer_path.to_string_lossy().into_owned();
    let file_size = meta.len();
    let file_modified_at = file_mtime_rfc3339(&meta);
    let size_i64 = i64::try_from(file_size).map_err(|_| {
        MetadataStoreError::Validation("trainer file size exceeds i64 range".to_string())
    })?;

    if let Some(raw) = community_trainer_sha256.filter(|s| !s.trim().is_empty()) {
        if let Some(expected) = normalize_sha256_hex(raw) {
            if expected != current_hash {
                outcome.community_advisory = Some(TrainerHashCommunityAdvisory {
                    expected,
                    current: current_hash.clone(),
                });
            }
        }
    }

    let row = lookup_trainer_hash_cache(conn, profile_id, &file_path)?;
    let now = Utc::now().to_rfc3339();

    match row {
        None => {
            let cache_id = Uuid::new_v4().simple().to_string();
            upsert_trainer_hash_cache(
                conn,
                &cache_id,
                profile_id,
                &file_path,
                Some(size_i64),
                file_modified_at.as_deref(),
                &current_hash,
                &now,
                &now,
                &now,
            )?;
            outcome.baseline = TrainerHashBaselineResult::FirstBaselineRecorded;
        }
        Some(row) => {
            let stats_match = row.file_size == Some(size_i64)
                && file_modified_at.as_deref() == row.file_modified_at.as_deref();
            if stats_match {
                if row.sha256_hash == current_hash {
                    outcome.baseline = TrainerHashBaselineResult::OkMatched;
                } else {
                    outcome.baseline = TrainerHashBaselineResult::Mismatch {
                        stored_hash: row.sha256_hash,
                        current_hash,
                    };
                }
            } else if row.sha256_hash == current_hash {
                upsert_trainer_hash_cache(
                    conn,
                    &row.cache_id,
                    profile_id,
                    &file_path,
                    Some(size_i64),
                    file_modified_at.as_deref(),
                    &current_hash,
                    &now,
                    &row.created_at,
                    &now,
                )?;
                outcome.baseline = TrainerHashBaselineResult::OkMatched;
            } else {
                outcome.baseline = TrainerHashBaselineResult::Mismatch {
                    stored_hash: row.sha256_hash,
                    current_hash,
                };
            }
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod normalize_tests {
    use super::normalize_sha256_hex;

    #[test]
    fn normalize_sha256_hex_accepts_0x_prefix() {
        let h = "ab".repeat(32);
        assert_eq!(
            normalize_sha256_hex(&format!("0x{h}")).as_deref(),
            Some(h.as_str())
        );
    }
}
