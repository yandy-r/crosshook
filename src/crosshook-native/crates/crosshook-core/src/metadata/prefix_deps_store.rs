use super::models::PrefixDependencyStateRow;
use super::MetadataStoreError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

pub fn upsert_dependency_state(
    conn: &Connection,
    profile_id: &str,
    package_name: &str,
    prefix_path: &str,
    state: &str,
    error: Option<&str>,
) -> Result<(), MetadataStoreError> {
    let now = Utc::now().to_rfc3339();
    // Use INSERT OR REPLACE on the unique constraint (profile_id, package_name, prefix_path).
    // Preserve original created_at if the row already exists.
    let existing_created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM prefix_dependency_state
             WHERE profile_id = ?1 AND package_name = ?2 AND prefix_path = ?3",
            params![profile_id, package_name, prefix_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "check existing prefix dep state",
            source,
        })?;

    let created_at = existing_created_at.unwrap_or_else(|| now.clone());
    let installed_at = if state == "installed" {
        Some(now.clone())
    } else {
        // Preserve existing installed_at if not changing to installed
        conn.query_row(
            "SELECT installed_at FROM prefix_dependency_state
             WHERE profile_id = ?1 AND package_name = ?2 AND prefix_path = ?3",
            params![profile_id, package_name, prefix_path],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "read existing installed_at",
            source,
        })?
        .flatten()
    };

    conn.execute(
        "INSERT OR REPLACE INTO prefix_dependency_state
         (profile_id, package_name, prefix_path, state, checked_at, installed_at, last_error, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            profile_id,
            package_name,
            prefix_path,
            state,
            Some(&now),
            installed_at,
            error,
            created_at,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert prefix dependency state",
        source,
    })?;

    Ok(())
}

pub fn load_dependency_states(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<PrefixDependencyStateRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, package_name, prefix_path, state,
                    checked_at, installed_at, last_error, created_at, updated_at
             FROM prefix_dependency_state
             WHERE profile_id = ?1
             ORDER BY package_name",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare load prefix dep states",
            source,
        })?;

    let rows = stmt
        .query_map(params![profile_id], |row| {
            Ok(PrefixDependencyStateRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                package_name: row.get(2)?,
                prefix_path: row.get(3)?,
                state: row.get(4)?,
                checked_at: row.get(5)?,
                installed_at: row.get(6)?,
                last_error: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query prefix dep states",
            source,
        })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect prefix dep state rows",
            source,
        })
}

pub fn load_dependency_state(
    conn: &Connection,
    profile_id: &str,
    package_name: &str,
    prefix_path: &str,
) -> Result<Option<PrefixDependencyStateRow>, MetadataStoreError> {
    conn.query_row(
        "SELECT id, profile_id, package_name, prefix_path, state,
                checked_at, installed_at, last_error, created_at, updated_at
         FROM prefix_dependency_state
         WHERE profile_id = ?1 AND package_name = ?2 AND prefix_path = ?3",
        params![profile_id, package_name, prefix_path],
        |row| {
            Ok(PrefixDependencyStateRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                package_name: row.get(2)?,
                prefix_path: row.get(3)?,
                state: row.get(4)?,
                checked_at: row.get(5)?,
                installed_at: row.get(6)?,
                last_error: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "load single prefix dep state",
        source,
    })
}

pub fn clear_dependency_states(
    conn: &Connection,
    profile_id: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "DELETE FROM prefix_dependency_state WHERE profile_id = ?1",
        params![profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "clear prefix dep states",
        source,
    })?;
    Ok(())
}

pub fn clear_stale_states(conn: &Connection, ttl_hours: i64) -> Result<u64, MetadataStoreError> {
    let cutoff = (Utc::now() - chrono::Duration::hours(ttl_hours)).to_rfc3339();
    let count = conn
        .execute(
            "DELETE FROM prefix_dependency_state WHERE checked_at < ?1",
            params![cutoff],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "clear stale prefix dep states",
            source,
        })?;
    Ok(count as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;
    use crate::metadata::migrations::run_migrations;

    fn setup_db_with_profile(profile_id: &str) -> Connection {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO profiles (profile_id, current_filename, current_path, game_name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))",
            params![profile_id, format!("{profile_id}.toml"), format!("/tmp/{profile_id}.toml"), "Test Game"],
        ).unwrap();
        conn
    }

    #[test]
    fn upsert_and_load_round_trip() {
        let conn = setup_db_with_profile("prof-1");
        upsert_dependency_state(&conn, "prof-1", "vcrun2019", "/tmp/pfx", "installed", None)
            .unwrap();
        let rows = load_dependency_states(&conn, "prof-1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].package_name, "vcrun2019");
        assert_eq!(rows[0].state, "installed");
        assert!(rows[0].installed_at.is_some());
    }

    #[test]
    fn upsert_overwrites_existing() {
        let conn = setup_db_with_profile("prof-2");
        upsert_dependency_state(&conn, "prof-2", "vcrun2019", "/tmp/pfx", "missing", None).unwrap();
        upsert_dependency_state(&conn, "prof-2", "vcrun2019", "/tmp/pfx", "installed", None)
            .unwrap();
        let rows = load_dependency_states(&conn, "prof-2").unwrap();
        assert_eq!(rows.len(), 1, "should have exactly one row after upsert");
        assert_eq!(rows[0].state, "installed");
    }

    #[test]
    fn clear_stale_removes_old_entries() {
        let conn = setup_db_with_profile("prof-3");
        // Insert with an old checked_at timestamp
        let old_time = (Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        conn.execute(
            "INSERT INTO prefix_dependency_state
             (profile_id, package_name, prefix_path, state, checked_at, created_at, updated_at)
             VALUES ('prof-3', 'dotnet48', '/tmp/pfx', 'installed', ?1, ?1, ?1)",
            params![old_time],
        )
        .unwrap();

        let deleted = clear_stale_states(&conn, 24).unwrap();
        assert_eq!(deleted, 1);
        let rows = load_dependency_states(&conn, "prof-3").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn cascade_delete_on_profile_removal() {
        let conn = setup_db_with_profile("prof-4");
        upsert_dependency_state(&conn, "prof-4", "vcrun2019", "/tmp/pfx", "installed", None)
            .unwrap();
        conn.execute("DELETE FROM profiles WHERE profile_id = 'prof-4'", [])
            .unwrap();
        let rows = load_dependency_states(&conn, "prof-4").unwrap();
        assert!(rows.is_empty(), "dep states should be cascade-deleted");
    }

    #[test]
    fn load_single_dependency_state() {
        let conn = setup_db_with_profile("prof-5");
        upsert_dependency_state(&conn, "prof-5", "vcrun2019", "/tmp/pfx", "installed", None)
            .unwrap();
        upsert_dependency_state(
            &conn,
            "prof-5",
            "dotnet48",
            "/tmp/pfx",
            "missing",
            Some("download failed"),
        )
        .unwrap();
        let row = load_dependency_state(&conn, "prof-5", "dotnet48", "/tmp/pfx").unwrap();
        assert!(row.is_some());
        let row = row.unwrap();
        assert_eq!(row.state, "missing");
        assert_eq!(row.last_error.as_deref(), Some("download failed"));
    }
}
