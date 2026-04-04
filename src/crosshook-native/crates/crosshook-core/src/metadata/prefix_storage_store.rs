use super::models::{PrefixStorageCleanupAuditRow, PrefixStorageSnapshotRow};
use super::MetadataStoreError;
use rusqlite::{params, Connection};

pub fn insert_snapshot(
    conn: &Connection,
    row: &PrefixStorageSnapshotRow,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT OR REPLACE INTO prefix_storage_snapshots
         (id, resolved_prefix_path, total_bytes, staged_trainers_bytes,
          is_orphan, referenced_profiles_json, stale_staged_count, scanned_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.id,
            row.resolved_prefix_path,
            row.total_bytes,
            row.staged_trainers_bytes,
            row.is_orphan as i32,
            row.referenced_profiles_json,
            row.stale_staged_count,
            row.scanned_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert prefix storage snapshot",
        source,
    })?;
    Ok(())
}

pub fn list_latest_snapshots(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<PrefixStorageSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, resolved_prefix_path, total_bytes, staged_trainers_bytes,
                    is_orphan, referenced_profiles_json, stale_staged_count, scanned_at
             FROM prefix_storage_snapshots
             ORDER BY scanned_at DESC
             LIMIT ?1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list prefix storage snapshots",
            source,
        })?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(PrefixStorageSnapshotRow {
                id: row.get(0)?,
                resolved_prefix_path: row.get(1)?,
                total_bytes: row.get(2)?,
                staged_trainers_bytes: row.get(3)?,
                is_orphan: row.get::<_, i32>(4)? != 0,
                referenced_profiles_json: row.get(5)?,
                stale_staged_count: row.get(6)?,
                scanned_at: row.get(7)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query prefix storage snapshots",
            source,
        })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect prefix storage snapshot rows",
            source,
        })
}

pub fn insert_cleanup_audit(
    conn: &Connection,
    row: &PrefixStorageCleanupAuditRow,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT INTO prefix_storage_cleanup_audit
         (id, target_kind, resolved_prefix_path, target_path, result, reason, reclaimed_bytes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.id,
            row.target_kind,
            row.resolved_prefix_path,
            row.target_path,
            row.result,
            row.reason,
            row.reclaimed_bytes,
            row.created_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert prefix storage cleanup audit",
        source,
    })?;
    Ok(())
}

pub fn list_cleanup_audit(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<PrefixStorageCleanupAuditRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, target_kind, resolved_prefix_path, target_path,
                    result, reason, reclaimed_bytes, created_at
             FROM prefix_storage_cleanup_audit
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list prefix storage cleanup audit",
            source,
        })?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(PrefixStorageCleanupAuditRow {
                id: row.get(0)?,
                target_kind: row.get(1)?,
                resolved_prefix_path: row.get(2)?,
                target_path: row.get(3)?,
                result: row.get(4)?,
                reason: row.get(5)?,
                reclaimed_bytes: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query prefix storage cleanup audit",
            source,
        })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect prefix storage cleanup audit rows",
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;
    use crate::metadata::migrations::run_migrations;

    fn setup_db() -> Connection {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn snapshot_insert_and_list_round_trip() {
        let conn = setup_db();
        let row = PrefixStorageSnapshotRow {
            id: "snap-1".into(),
            resolved_prefix_path: "/home/user/.steam/pfx".into(),
            total_bytes: 1_000_000,
            staged_trainers_bytes: 200_000,
            is_orphan: false,
            referenced_profiles_json: r#"["game.toml"]"#.into(),
            stale_staged_count: 2,
            scanned_at: "2026-04-04T12:00:00Z".into(),
        };
        insert_snapshot(&conn, &row).unwrap();

        let rows = list_latest_snapshots(&conn, 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "snap-1");
        assert_eq!(rows[0].resolved_prefix_path, "/home/user/.steam/pfx");
        assert_eq!(rows[0].total_bytes, 1_000_000);
        assert_eq!(rows[0].staged_trainers_bytes, 200_000);
        assert!(!rows[0].is_orphan);
        assert_eq!(rows[0].stale_staged_count, 2);
    }

    #[test]
    fn audit_insert_and_list_round_trip() {
        let conn = setup_db();
        let row = PrefixStorageCleanupAuditRow {
            id: "audit-1".into(),
            target_kind: "orphan_prefix".into(),
            resolved_prefix_path: "/home/user/.steam/pfx".into(),
            target_path: "/home/user/.steam/pfx".into(),
            result: "deleted".into(),
            reason: None,
            reclaimed_bytes: 0,
            created_at: "2026-04-04T12:00:00Z".into(),
        };
        insert_cleanup_audit(&conn, &row).unwrap();

        let rows = list_cleanup_audit(&conn, 100).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "audit-1");
        assert_eq!(rows[0].target_kind, "orphan_prefix");
        assert_eq!(rows[0].result, "deleted");
        assert!(rows[0].reason.is_none());
    }

    #[test]
    fn list_respects_limit() {
        let conn = setup_db();
        for i in 0..5 {
            insert_snapshot(
                &conn,
                &PrefixStorageSnapshotRow {
                    id: format!("snap-{i}"),
                    resolved_prefix_path: "/pfx".into(),
                    total_bytes: 100,
                    staged_trainers_bytes: 0,
                    is_orphan: false,
                    referenced_profiles_json: "[]".into(),
                    stale_staged_count: 0,
                    scanned_at: format!("2026-04-04T12:0{i}:00Z"),
                },
            )
            .unwrap();
        }
        let rows = list_latest_snapshots(&conn, 3).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn empty_list_on_fresh_db() {
        let conn = setup_db();
        let snapshots = list_latest_snapshots(&conn, 50).unwrap();
        assert!(snapshots.is_empty());
        let audit = list_cleanup_audit(&conn, 100).unwrap();
        assert!(audit.is_empty());
    }
}
