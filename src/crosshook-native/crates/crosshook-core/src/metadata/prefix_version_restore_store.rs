use super::models::PrefixVersionRestoreJournalRow;
use super::MetadataStoreError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

pub fn upsert_restore(
    conn: &Connection,
    record: &PrefixVersionRestoreJournalRow,
) -> Result<(), MetadataStoreError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO prefix_version_restore_journal
         (prefix_path, profile_id, binary_path, tool_type, steam_app_id,
          restore_verb, state, last_error, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', NULL, ?7, ?7)
         ON CONFLICT(prefix_path) DO UPDATE SET
             profile_id = excluded.profile_id,
             binary_path = excluded.binary_path,
             tool_type = excluded.tool_type,
             steam_app_id = excluded.steam_app_id,
             restore_verb = excluded.restore_verb,
             state = 'pending',
             last_error = NULL,
             updated_at = excluded.updated_at",
        params![
            record.prefix_path,
            record.profile_id,
            record.binary_path,
            record.tool_type,
            record.steam_app_id,
            record.restore_verb,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert prefix version restore journal",
        source,
    })?;
    Ok(())
}

const SELECT_COLUMNS: &str = "SELECT prefix_path, profile_id, binary_path, tool_type, steam_app_id,
        restore_verb, state, last_error, created_at, updated_at
 FROM prefix_version_restore_journal";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrefixVersionRestoreJournalRow> {
    Ok(PrefixVersionRestoreJournalRow {
        prefix_path: row.get(0)?,
        profile_id: row.get(1)?,
        binary_path: row.get(2)?,
        tool_type: row.get(3)?,
        steam_app_id: row.get(4)?,
        restore_verb: row.get(5)?,
        state: row.get(6)?,
        last_error: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn load_restore(
    conn: &Connection,
    prefix_path: &str,
) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
    conn.query_row(
        &format!("{SELECT_COLUMNS} WHERE prefix_path = ?1"),
        params![prefix_path],
        map_row,
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "load prefix version restore journal",
        source,
    })
}

/// Read-only listing of every journal row, ordered by `prefix_path`.
pub fn list_restores(
    conn: &Connection,
) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
    let map_err = |source| MetadataStoreError::Database {
        action: "list prefix version restore journal",
        source,
    };
    let mut stmt = conn
        .prepare(&format!("{SELECT_COLUMNS} ORDER BY prefix_path"))
        .map_err(map_err)?;
    let rows = stmt.query_map([], map_row).map_err(map_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_err)
}

pub fn mark_restore_failed(
    conn: &Connection,
    prefix_path: &str,
    error: &str,
) -> Result<(), MetadataStoreError> {
    let now = Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            "UPDATE prefix_version_restore_journal
         SET state = 'failed', last_error = ?2, updated_at = ?3
         WHERE prefix_path = ?1",
            params![prefix_path, error, now],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "mark prefix version restore failed",
            source,
        })?;
    if affected != 1 {
        return Err(MetadataStoreError::Validation(
            "cannot mark prefix repair failed because no journal row exists".to_string(),
        ));
    }
    Ok(())
}

pub fn delete_restore(conn: &Connection, prefix_path: &str) -> Result<(), MetadataStoreError> {
    let affected = conn
        .execute(
            "DELETE FROM prefix_version_restore_journal WHERE prefix_path = ?1",
            params![prefix_path],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "delete prefix version restore journal",
            source,
        })?;
    if affected != 1 {
        return Err(MetadataStoreError::Validation(
            "cannot delete prefix repair because no journal row exists".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;
    use crate::metadata::migrations::run_migrations;
    use crate::metadata::test_support::insert_test_profile_row;

    #[test]
    fn journal_round_trip_failure_and_delete() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        insert_test_profile_row(&conn, "profile-1");
        let record = PrefixVersionRestoreJournalRow {
            prefix_path: "/games/pfx".to_string(),
            profile_id: Some("profile-1".to_string()),
            binary_path: "winetricks".to_string(),
            tool_type: "winetricks".to_string(),
            steam_app_id: None,
            restore_verb: "win10".to_string(),
            state: "pending".to_string(),
            last_error: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        upsert_restore(&conn, &record).unwrap();
        let loaded = load_restore(&conn, "/games/pfx").unwrap().unwrap();
        assert_eq!(loaded.restore_verb, "win10");
        assert_eq!(loaded.state, "pending");

        mark_restore_failed(&conn, "/games/pfx", "restore timed out").unwrap();
        let failed = load_restore(&conn, "/games/pfx").unwrap().unwrap();
        assert_eq!(failed.state, "failed");
        assert_eq!(failed.last_error.as_deref(), Some("restore timed out"));

        delete_restore(&conn, "/games/pfx").unwrap();
        assert!(load_restore(&conn, "/games/pfx").unwrap().is_none());
    }

    #[test]
    fn missing_restore_row_rejects_unsafe_mutations() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let mark = mark_restore_failed(&conn, "/games/missing/pfx", "restore failed");
        assert!(mark.unwrap_err().to_string().contains("no journal row"));

        let delete = delete_restore(&conn, "/games/missing/pfx");
        assert!(delete.unwrap_err().to_string().contains("no journal row"));
    }

    fn journal_row(prefix_path: &str, verb: &str) -> PrefixVersionRestoreJournalRow {
        PrefixVersionRestoreJournalRow {
            prefix_path: prefix_path.to_string(),
            profile_id: None,
            binary_path: "/usr/bin/winetricks".to_string(),
            tool_type: "winetricks".to_string(),
            steam_app_id: None,
            restore_verb: verb.to_string(),
            state: "pending".to_string(),
            last_error: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn list_restores_returns_rows_sorted_by_prefix_path() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        upsert_restore(&conn, &journal_row("/games/zeta/pfx", "win7")).unwrap();
        upsert_restore(&conn, &journal_row("/games/alpha/pfx", "win10")).unwrap();
        mark_restore_failed(&conn, "/games/zeta/pfx", "restore failed").unwrap();

        let rows = list_restores(&conn).unwrap();

        let summary: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row.prefix_path.as_str(),
                    row.restore_verb.as_str(),
                    row.state.as_str(),
                    row.last_error.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("/games/alpha/pfx", "win10", "pending", None),
                ("/games/zeta/pfx", "win7", "failed", Some("restore failed")),
            ]
        );
    }

    #[test]
    fn list_restores_on_empty_table_returns_empty_vec() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert!(list_restores(&conn).unwrap().is_empty());
    }

    #[test]
    fn store_lists_prefix_version_restores() {
        let store = crate::metadata::MetadataStore::open_in_memory().unwrap();
        store
            .upsert_prefix_version_restore(&journal_row("/games/b/pfx", "win10"))
            .unwrap();
        store
            .upsert_prefix_version_restore(&journal_row("/games/a/pfx", "win10"))
            .unwrap();

        let paths: Vec<_> = store
            .list_prefix_version_restores()
            .unwrap()
            .into_iter()
            .map(|row| row.prefix_path)
            .collect();
        assert_eq!(paths, vec!["/games/a/pfx", "/games/b/pfx"]);
    }

    #[test]
    fn disabled_store_list_fails_closed_like_load() {
        let store = crate::metadata::MetadataStore::disabled();
        assert!(matches!(
            store.load_prefix_version_restore("/prefix"),
            Err(MetadataStoreError::ReadOnlyDisabled { .. })
        ));
        assert!(matches!(
            store.list_prefix_version_restores(),
            Err(MetadataStoreError::ReadOnlyDisabled { .. })
        ));
    }
}
