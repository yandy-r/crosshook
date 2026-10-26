use std::ffi::OsString;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use tempfile::{tempdir, TempDir};

use super::{
    read_library_view_mode, webkit_local_storage_path, WEBKIT_LIBRARY_VIEW_MODE_KEY,
    WEBKIT_LOCAL_STORAGE_FILE, WEBKIT_MAX_FILE_BYTES,
};
use crate::app_id_migration::CURRENT_TAURI_APP_ID_DIR;
use crate::settings::LibraryViewMode;

fn utf16le(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn database(row: Option<(&str, &[u8])>) -> (TempDir, PathBuf) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join(WEBKIT_LOCAL_STORAGE_FILE);
    let connection = Connection::open(&path).expect("open fixture");
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .expect("WAL fixture");
    connection
        .execute_batch(
            "CREATE TABLE ItemTable (\
             key TEXT UNIQUE ON CONFLICT REPLACE, \
             value BLOB NOT NULL ON CONFLICT FAIL)",
        )
        .expect("create ItemTable");
    if let Some((key, value)) = row {
        connection
            .execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .expect("insert fixture value");
    }
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .expect("checkpoint fixture");
    drop(connection);
    (dir, path)
}

fn directory_names(path: &Path) -> Vec<OsString> {
    let mut names = fs::read_dir(path)
        .expect("read fixture directory")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn imports_utf16le_list() {
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &utf16le("list"))));
    assert_eq!(read_library_view_mode(&path), Some(LibraryViewMode::List));
}

#[test]
fn imports_utf16le_grid() {
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &utf16le("grid"))));
    assert_eq!(read_library_view_mode(&path), Some(LibraryViewMode::Grid));
}

#[test]
fn missing_file_returns_none() {
    let dir = tempdir().expect("tempdir");
    assert_eq!(read_library_view_mode(&dir.path().join("missing")), None);
}

#[test]
fn corrupt_file_returns_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("corrupt.localstorage");
    fs::write(&path, [0xde, 0xad, 0xbe, 0xef].repeat(128)).expect("write corrupt fixture");
    assert_eq!(read_library_view_mode(&path), None);
}

#[cfg(unix)]
#[test]
fn symlink_returns_none() {
    let (dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &utf16le("list"))));
    let link = dir.path().join("linked.localstorage");
    std::os::unix::fs::symlink(&path, &link).expect("symlink");
    assert_eq!(read_library_view_mode(&link), None);
}

#[cfg(unix)]
#[test]
fn fifo_returns_none_without_blocking() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("fifo.localstorage");
    nix::unistd::mkfifo(&path, nix::sys::stat::Mode::from_bits_truncate(0o600)).expect("FIFO");
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn sparse_oversize_file_returns_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("oversize.localstorage");
    let file = File::create(&path).expect("create sparse fixture");
    file.set_len(WEBKIT_MAX_FILE_BYTES + 1)
        .expect("grow sparse fixture");
    drop(file);
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn item_table_with_generated_value_returns_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("generated.localstorage");
    let connection = Connection::open(&path).expect("open fixture");
    connection
        .execute_batch(
            "CREATE TABLE ItemTable (\
             key TEXT, \
             value BLOB GENERATED ALWAYS AS (X'6C00690073007400') VIRTUAL);\
             INSERT INTO ItemTable (key) VALUES ('crosshook.library.viewMode')",
        )
        .expect("create generated-column fixture");
    drop(connection);

    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn item_table_view_returns_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("view.localstorage");
    let connection = Connection::open(&path).expect("open fixture");
    connection
        .execute_batch(
            "CREATE VIEW ItemTable AS SELECT \
             'crosshook.library.viewMode' AS key, X'6C00690073007400' AS value",
        )
        .expect("create view fixture");
    drop(connection);
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn item_table_virtual_table_returns_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("fts.localstorage");
    let connection = Connection::open(&path).expect("open fixture");
    connection
        .execute_batch("CREATE VIRTUAL TABLE ItemTable USING fts5(key, value)")
        .expect("create virtual table fixture");
    drop(connection);
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn wrong_table_name_returns_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("wrong-schema.localstorage");
    let connection = Connection::open(&path).expect("open fixture");
    connection
        .execute_batch("CREATE TABLE OtherTable (key TEXT, value BLOB)")
        .expect("create schema fixture");
    drop(connection);
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn absent_key_returns_none() {
    let (_dir, path) = database(None);
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn odd_length_value_returns_none() {
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &[b'l', 0, b'i'])));
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn unsupported_value_returns_none() {
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &utf16le("tiles"))));
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn utf8_value_returns_none() {
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, b"list")));
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn invalid_utf16_returns_none() {
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &[0x00, 0xd8])));
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn oversized_value_returns_none() {
    let value = utf16le("list-with-padding");
    let (_dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &value)));
    assert_eq!(read_library_view_mode(&path), None);
}

#[test]
fn reserved_uri_characters_in_path_are_encoded() {
    let (dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &utf16le("list"))));
    let reserved_path = dir.path().join("classic%20?view#1.localstorage");
    fs::rename(&path, &reserved_path).expect("rename fixture");
    assert_eq!(
        read_library_view_mode(&reserved_path),
        Some(LibraryViewMode::List)
    );
}

#[test]
fn successful_read_preserves_database_and_creates_no_sidecars() {
    let (dir, path) = database(Some((WEBKIT_LIBRARY_VIEW_MODE_KEY, &utf16le("list"))));
    let before_bytes = fs::read(&path).expect("read fixture bytes");
    let before_mtime = fs::metadata(&path)
        .expect("fixture metadata")
        .modified()
        .expect("mtime");
    let before_entries = directory_names(dir.path());

    assert_eq!(read_library_view_mode(&path), Some(LibraryViewMode::List));

    assert_eq!(fs::read(&path).expect("read fixture bytes"), before_bytes);
    assert_eq!(
        fs::metadata(&path)
            .expect("fixture metadata")
            .modified()
            .expect("mtime"),
        before_mtime
    );
    assert_eq!(directory_names(dir.path()), before_entries);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        assert!(
            !PathBuf::from(sidecar).exists(),
            "unexpected {suffix} sidecar"
        );
    }
}

#[test]
fn platform_path_uses_current_classic_storage_directory() {
    let path = webkit_local_storage_path().expect("platform base directories");
    assert!(path.ends_with(
        Path::new(CURRENT_TAURI_APP_ID_DIR)
            .join("localstorage")
            .join(WEBKIT_LOCAL_STORAGE_FILE)
    ));
}
