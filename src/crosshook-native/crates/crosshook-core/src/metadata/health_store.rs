use super::MetadataStoreError;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct HealthSnapshotRow {
    pub profile_id: String,
    pub profile_name: String,
    pub status: String,
    pub issue_count: i64,
    pub checked_at: String,
}

pub fn upsert_health_snapshot(
    conn: &Connection,
    profile_id: &str,
    status: &str,
    issue_count: usize,
    checked_at: &str,
) -> Result<(), MetadataStoreError> {
    let checked_issue_count = i64::try_from(issue_count).map_err(|_| {
        MetadataStoreError::Validation("health snapshot issue_count exceeds i64 range".to_string())
    })?;

    conn.execute(
        "INSERT OR REPLACE INTO health_snapshots (profile_id, status, issue_count, checked_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![profile_id, status, checked_issue_count, checked_at],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a health snapshot row",
        source,
    })?;

    Ok(())
}

pub fn load_health_snapshots(
    conn: &Connection,
) -> Result<Vec<HealthSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT hs.profile_id, p.current_filename, hs.status, hs.issue_count, hs.checked_at
             FROM health_snapshots hs
             INNER JOIN profiles p ON hs.profile_id = p.profile_id
             WHERE p.deleted_at IS NULL",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare load health snapshots query",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(HealthSnapshotRow {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                status: row.get(2)?,
                issue_count: row.get(3)?,
                checked_at: row.get(4)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query health snapshots",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect health snapshot rows",
            source,
        })?;

    Ok(rows)
}

pub fn lookup_health_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<HealthSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT hs.profile_id, p.current_filename, hs.status, hs.issue_count, hs.checked_at
             FROM health_snapshots hs
             INNER JOIN profiles p ON hs.profile_id = p.profile_id
             WHERE hs.profile_id = ?1 AND p.deleted_at IS NULL",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare lookup health snapshot query",
            source,
        })?;

    stmt.query_row(params![profile_id], |row| {
        Ok(HealthSnapshotRow {
            profile_id: row.get(0)?,
            profile_name: row.get(1)?,
            status: row.get(2)?,
            issue_count: row.get(3)?,
            checked_at: row.get(4)?,
        })
    })
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "lookup health snapshot by profile_id",
        source,
    })
}
