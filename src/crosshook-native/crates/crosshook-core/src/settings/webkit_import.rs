//! Best-effort import of Classic's WebKit local storage.
//!
//! `webkit2gtk-4.1` stores the setting as UTF-16LE in an SQLite database. The
//! immutable read avoids locks and journal sidecar creation. Consequently, an
//! un-checkpointed WAL is intentionally ignored and may leave an older value.

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use tracing::debug;

use super::LibraryViewMode;
use crate::app_id_migration::CURRENT_TAURI_APP_ID_DIR;
use crate::platform::base_dirs;

/// Filename used by WebKit for the Classic origin's local storage.
pub const WEBKIT_LOCAL_STORAGE_FILE: &str = "tauri_localhost_0.localstorage";
/// Classic local-storage key for the library layout.
pub const WEBKIT_LIBRARY_VIEW_MODE_KEY: &str = "crosshook.library.viewMode";
/// Maximum accepted local-storage database size.
pub const WEBKIT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

const MAX_VALUE_BYTES: usize = 16;

/// Return Classic's WebKit local-storage database path, if base dirs exist.
pub fn webkit_local_storage_path() -> Option<PathBuf> {
    Some(
        base_dirs()?
            .data_local_dir()
            .join(CURRENT_TAURI_APP_ID_DIR)
            .join("localstorage")
            .join(WEBKIT_LOCAL_STORAGE_FILE),
    )
}

/// Read Classic's library layout without modifying or locking the database.
///
/// Every failure returns `None`; imports are optional and never user-facing
/// errors. Only a regular file within the size limit is accepted.
///
/// TOCTOU hardening: the path is first opened with
/// `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` and that descriptor is
/// `fstat`'d, so a FIFO swapped in after a prior `lstat` can no longer hang
/// the open (the `O_NONBLOCK` open of a reader-less FIFO fails with `ENXIO`)
/// and a post-swap symlink is refused. `O_NONBLOCK` is a read-only flag with
/// no effect on regular files. The descriptor is then closed and SQLite
/// re-opens the path read-only and immutable, so an un-checkpointed WAL is
/// intentionally ignored and may leave an older value.
///
/// Residual window: between the pre-check and SQLite's own open, an attacker
/// able to replace the path could still swap in a FIFO or grow the file past
/// the cap. Closing it fully would require SQLite to consume an existing
/// descriptor; rusqlite exposes no such API, and its `deserialize` path
/// cannot query databases whose header declares WAL mode (queries fail with
/// `SQLITE_CANTOPEN`), so this pre-check is the strongest available
/// hardening without new dependencies.
pub fn read_library_view_mode(path: &Path) -> Option<LibraryViewMode> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return reject("open_failed"),
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return reject("metadata_unavailable"),
    };
    if !metadata.file_type().is_file() {
        return reject("not_regular_file");
    }
    if metadata.len() > WEBKIT_MAX_FILE_BYTES {
        return reject("file_too_large");
    }
    drop(file);

    let uri = format!("file:{}?mode=ro&immutable=1", percent_encode_path(path));
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = match Connection::open_with_flags(uri, flags) {
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

/// Import the setting from the platform's Classic data directory.
pub fn import_library_view_mode() -> Option<LibraryViewMode> {
    webkit_local_storage_path().and_then(|path| read_library_view_mode(&path))
}

fn percent_encode_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = path.as_os_str().as_bytes();
    let mut encoded = String::with_capacity(bytes.len() * 3);
    for &byte in bytes {
        if matches!(byte, b'%' | b'?' | b'#') || !byte.is_ascii() {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        } else {
            encoded.push(char::from(byte));
        }
    }
    encoded
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
