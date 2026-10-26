//! Best-effort import of Classic's WebKit local storage.
//!
//! `webkit2gtk-4.1` stores the setting as UTF-16LE in an SQLite database. Both
//! the database and any WAL are snapshotted before SQLite opens the private
//! copy, so importing never modifies or locks Classic's original files.

use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use tracing::debug;

use super::LibraryViewMode;
use crate::app_id_migration::{CURRENT_TAURI_APP_ID_DIR, LEGACY_TAURI_APP_ID_DIR};
use crate::platform::base_dirs;

/// Filename used by WebKit for the Classic origin's local storage.
pub const WEBKIT_LOCAL_STORAGE_FILE: &str = "tauri_localhost_0.localstorage";
/// Classic local-storage key for the library layout.
pub const WEBKIT_LIBRARY_VIEW_MODE_KEY: &str = "crosshook.library.viewMode";
/// Maximum accepted local-storage database size.
pub const WEBKIT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

const MAX_VALUE_BYTES: usize = 16;

/// Return Classic's current and legacy WebKit database candidates, in order.
pub fn webkit_local_storage_paths() -> Vec<PathBuf> {
    let Some(dirs) = base_dirs() else {
        return Vec::new();
    };
    [CURRENT_TAURI_APP_ID_DIR, LEGACY_TAURI_APP_ID_DIR]
        .into_iter()
        .map(|app_id| {
            dirs.data_local_dir()
                .join(app_id)
                .join("localstorage")
                .join(WEBKIT_LOCAL_STORAGE_FILE)
        })
        .collect()
}

/// Read Classic's library layout without modifying or locking its files.
///
/// Every failure returns `None`; imports are optional and never user-facing
/// errors. The database and optional `-wal` must each be regular files within
/// the size limit. Each is opened with
/// `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`, checked using `fstat`, and
/// copied from that same descriptor to a private temporary directory. Symlinked
/// files and FIFOs are refused, while symlinked parent directories are allowed.
/// SQLite opens only the private copy, without `immutable=1`, so it reads WAL
/// values and may create its own `-shm` without touching the original files.
pub fn read_library_view_mode(path: &Path) -> Option<LibraryViewMode> {
    let snapshot = match tempfile::tempdir() {
        Ok(snapshot) => snapshot,
        Err(_) => return reject("tempdir_failed"),
    };
    let snapshot_path = snapshot.path().join(WEBKIT_LOCAL_STORAGE_FILE);
    if let Err(reason) = snapshot_file(path, &snapshot_path, false) {
        return reject(reason);
    }
    if let Err(reason) = snapshot_file(&wal_path(path), &wal_path(&snapshot_path), true) {
        return reject(reason);
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = match Connection::open_with_flags(&snapshot_path, flags) {
        Ok(connection) => connection,
        Err(_) => return reject("open_failed"),
    };
    if connection
        .pragma_update(None, "trusted_schema", false)
        .is_err()
    {
        return reject("trusted_schema_disable_failed");
    }
    if connection.pragma_update(None, "query_only", true).is_err() {
        return reject("query_only_enable_failed");
    }

    // `rootpage > 0` excludes virtual tables (e.g. FTS5): their sqlite_schema
    // row reports type 'table' with rootpage 0, and their module code could
    // run on query despite trusted_schema = off.
    let item_table_shape = match connection
        .query_row(
            "SELECT type = 'table' AND rootpage > 0 FROM sqlite_schema \
             WHERE name = 'ItemTable'",
            [],
            |row| row.get::<_, bool>(0),
        )
        .optional()
    {
        Ok(item_table_shape) => item_table_shape,
        Err(_) => return reject("schema_query_failed"),
    };
    if item_table_shape != Some(true) {
        return reject("item_table_not_plain_table");
    }

    // `table_xinfo` also reports hidden/generated columns; `table_info` does
    // not, which could let malicious generated expressions execute on query.
    let item_table_columns = match connection.prepare("PRAGMA table_xinfo('ItemTable')") {
        Ok(mut statement) => match statement.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(6)?))
        }) {
            Ok(rows) => match rows.collect::<Result<Vec<_>, _>>() {
                Ok(columns) => columns,
                Err(_) => return reject("column_schema_query_failed"),
            },
            Err(_) => return reject("column_schema_query_failed"),
        },
        Err(_) => return reject("column_schema_query_failed"),
    };
    if !matches!(
        item_table_columns.as_slice(),
        [(key, 0), (value, 0)] if key == "key" && value == "value"
    ) {
        return reject("unexpected_item_table_columns");
    }

    let value = match connection
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1 AND length(value) <= 16 LIMIT 1",
            params![WEBKIT_LIBRARY_VIEW_MODE_KEY],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(_) => return reject("value_query_failed"),
    };
    let value = match value {
        Some(value) => value,
        None => return reject("key_absent"),
    };
    decode_view_mode(&value)
}

/// Import the first readable setting from Classic's current or legacy directory.
pub fn import_library_view_mode() -> Option<LibraryViewMode> {
    webkit_local_storage_paths()
        .iter()
        .find_map(|path| read_library_view_mode(path))
}

fn wal_path(path: &Path) -> PathBuf {
    let mut path = path.as_os_str().to_os_string();
    path.push("-wal");
    path.into()
}

fn snapshot_file(source: &Path, destination: &Path, optional: bool) -> Result<(), &'static str> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
        .open(source)
    {
        Ok(file) => file,
        Err(error) if optional && error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("open_failed"),
    };
    let metadata = file.metadata().map_err(|_| "metadata_unavailable")?;
    if !metadata.file_type().is_file() {
        return Err("not_regular_file");
    }
    if metadata.len() > WEBKIT_MAX_FILE_BYTES {
        return Err("file_too_large");
    }
    let mut copy = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(destination)
        .map_err(|_| "copy_create_failed")?;
    let copied = io::copy(&mut file.take(WEBKIT_MAX_FILE_BYTES + 1), &mut copy)
        .map_err(|_| "copy_failed")?;
    if copied > WEBKIT_MAX_FILE_BYTES {
        return Err("file_too_large");
    }
    Ok(())
}

fn decode_view_mode(bytes: &[u8]) -> Option<LibraryViewMode> {
    if bytes.len() > MAX_VALUE_BYTES || !bytes.len().is_multiple_of(2) {
        return reject("invalid_value_length");
    }
    let code_units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let value = match String::from_utf16(&code_units) {
        Ok(value) => value,
        Err(_) => return reject("invalid_utf16"),
    };
    match LibraryViewMode::parse(&value) {
        Some(mode) => Some(mode),
        None => reject("unknown_view_mode"),
    }
}

fn reject(reason: &'static str) -> Option<LibraryViewMode> {
    debug!(reason, "webkit library view import skipped");
    None
}

#[cfg(test)]
mod tests;
