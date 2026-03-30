use super::models::{
    MAX_VERSION_SNAPSHOTS_PER_PROFILE, VersionCorrelationStatus, VersionSnapshotRow,
};
use super::MetadataStoreError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub fn upsert_version_snapshot(
    conn: &Connection,
    profile_id: &str,
    steam_app_id: &str,
    steam_build_id: Option<&str>,
    trainer_version: Option<&str>,
    trainer_file_hash: Option<&str>,
    human_game_ver: Option<&str>,
    status: &str,
) -> Result<(), MetadataStoreError> {
    let checked_at = Utc::now().to_rfc3339();

    let tx =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|source| {
            MetadataStoreError::Database {
                action: "start a version snapshot transaction",
                source,
            }
        })?;

    tx.execute(
        "INSERT INTO version_snapshots
             (profile_id, steam_app_id, steam_build_id, trainer_version,
              trainer_file_hash, human_game_ver, status, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            profile_id,
            steam_app_id,
            steam_build_id,
            trainer_version,
            trainer_file_hash,
            human_game_ver,
            status,
            checked_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a version snapshot row",
        source,
    })?;

    tx.execute(
        "DELETE FROM version_snapshots
         WHERE profile_id = ?1
           AND id NOT IN (
               SELECT id FROM version_snapshots
               WHERE profile_id = ?1
               ORDER BY checked_at DESC
               LIMIT ?2
           )",
        params![profile_id, MAX_VERSION_SNAPSHOTS_PER_PROFILE as i64],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "prune old version snapshot rows",
        source,
    })?;

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the version snapshot transaction",
        source,
    })?;

    Ok(())
}

pub fn lookup_latest_version_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, steam_app_id, steam_build_id, trainer_version,
                    trainer_file_hash, human_game_ver, status, checked_at
             FROM version_snapshots
             WHERE profile_id = ?1
             ORDER BY checked_at DESC
             LIMIT 1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare lookup latest version snapshot query",
            source,
        })?;

    stmt.query_row(params![profile_id], |row| {
        Ok(VersionSnapshotRow {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            steam_app_id: row.get(2)?,
            steam_build_id: row.get(3)?,
            trainer_version: row.get(4)?,
            trainer_file_hash: row.get(5)?,
            human_game_ver: row.get(6)?,
            status: row.get(7)?,
            checked_at: row.get(8)?,
        })
    })
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "lookup latest version snapshot by profile_id",
        source,
    })
}

pub fn load_version_snapshots_for_profiles(
    conn: &Connection,
) -> Result<Vec<VersionSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, steam_app_id, steam_build_id, trainer_version,
                    trainer_file_hash, human_game_ver, status, checked_at
             FROM version_snapshots
             WHERE id IN (SELECT MAX(id) FROM version_snapshots GROUP BY profile_id)",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare load version snapshots for profiles query",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(VersionSnapshotRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                steam_app_id: row.get(2)?,
                steam_build_id: row.get(3)?,
                trainer_version: row.get(4)?,
                trainer_file_hash: row.get(5)?,
                human_game_ver: row.get(6)?,
                status: row.get(7)?,
                checked_at: row.get(8)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query version snapshots for profiles",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect version snapshot rows",
            source,
        })?;

    Ok(rows)
}

pub fn acknowledge_version_change(
    conn: &Connection,
    profile_id: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "UPDATE version_snapshots
         SET status = 'matched'
         WHERE id = (
             SELECT id FROM version_snapshots
             WHERE profile_id = ?1
             ORDER BY checked_at DESC
             LIMIT 1
         )",
        params![profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "acknowledge version change for profile",
        source,
    })?;

    Ok(())
}

/// Pure comparison function — no I/O. Determines the correlation status between
/// the current game build and the last recorded snapshot.
///
/// Returns `UpdateInProgress` when `state_flags` is `Some(n)` where `n != 4`
/// (Steam is actively updating the game). When `state_flags` is `None` (manifest
/// not found), proceeds normally. Returns `Untracked` when no snapshot exists.
/// Otherwise compares build ID and trainer hash to detect changes.
pub fn compute_correlation_status(
    current_build_id: &str,
    snapshot_build_id: Option<&str>,
    current_trainer_hash: Option<&str>,
    snapshot_trainer_hash: Option<&str>,
    state_flags: Option<u32>,
) -> VersionCorrelationStatus {
    if let Some(flags) = state_flags {
        if flags != 4 {
            return VersionCorrelationStatus::UpdateInProgress;
        }
    }

    let Some(snapshot_build) = snapshot_build_id else {
        return VersionCorrelationStatus::Untracked;
    };

    let build_changed = current_build_id != snapshot_build;
    let trainer_changed = current_trainer_hash != snapshot_trainer_hash;

    match (build_changed, trainer_changed) {
        (true, true) => VersionCorrelationStatus::BothChanged,
        (true, false) => VersionCorrelationStatus::GameUpdated,
        (false, true) => VersionCorrelationStatus::TrainerChanged,
        (false, false) => VersionCorrelationStatus::Matched,
    }
}

/// Read the file at `path`, compute its SHA-256 hash, and return the lowercase
/// hex digest. Returns `None` if the file cannot be read for any reason.
pub fn hash_trainer_file(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    Some(hex)
}
