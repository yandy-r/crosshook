use super::MetadataStoreError;
use rusqlite::Connection;
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub fn open_at_path(path: &Path) -> Result<Connection, MetadataStoreError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path).map_err(|source| MetadataStoreError::Io {
            action: "inspect the metadata database path",
            path: path.to_path_buf(),
            source,
        })?;

        if metadata.file_type().is_symlink() {
            return Err(MetadataStoreError::SymlinkDetected(path.to_path_buf()));
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| MetadataStoreError::Io {
            action: "create the metadata database directory",
            path: parent.to_path_buf(),
            source,
        })?;
        fs::set_permissions(parent, Permissions::from_mode(0o700)).map_err(|source| {
            MetadataStoreError::Io {
                action: "secure the metadata database directory permissions",
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    let conn = Connection::open(path).map_err(|source| MetadataStoreError::Database {
        action: "open the metadata database",
        source,
    })?;

    fs::set_permissions(path, Permissions::from_mode(0o600)).map_err(|source| {
        MetadataStoreError::Io {
            action: "secure the metadata database file permissions",
            path: path.to_path_buf(),
            source,
        }
    })?;

    configure_connection(&conn, true)?;

    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection, MetadataStoreError> {
    let conn = Connection::open_in_memory().map_err(|source| MetadataStoreError::Database {
        action: "open the in-memory metadata database",
        source,
    })?;

    configure_connection(&conn, false)?;

    Ok(conn)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn configure_connection(conn: &Connection, expect_wal: bool) -> Result<(), MetadataStoreError> {
    if expect_wal {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;\
             PRAGMA foreign_keys=ON;\
             PRAGMA synchronous=NORMAL;\
             PRAGMA busy_timeout=5000;\
             PRAGMA secure_delete=ON;",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "configure metadata database pragmas",
            source,
        })?;
    } else {
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;\
             PRAGMA synchronous=NORMAL;\
             PRAGMA busy_timeout=5000;\
             PRAGMA secure_delete=ON;",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "configure metadata database pragmas",
            source,
        })?;
    }

    let journal_mode = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "verify metadata database journal mode",
            source,
        })?;
    let expected_journal_mode = if expect_wal { "wal" } else { "memory" };
    if journal_mode.to_ascii_lowercase() != expected_journal_mode {
        return Err(MetadataStoreError::Corrupt(format!(
            "expected journal_mode {expected_journal_mode}, got {journal_mode}"
        )));
    }

    let foreign_keys = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "verify metadata database foreign key enforcement",
            source,
        })?;
    if foreign_keys != 1 {
        return Err(MetadataStoreError::Corrupt(format!(
            "expected foreign_keys to be enabled, got {foreign_keys}"
        )));
    }

    conn.pragma_update(None, "application_id", 0x43484B00_i32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set the metadata database application id",
            source,
        })?;

    let quick_check = conn
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "run the metadata database quick check",
            source,
        })?;
    if quick_check != "ok" {
        return Err(MetadataStoreError::Corrupt(quick_check));
    }

    Ok(())
}
