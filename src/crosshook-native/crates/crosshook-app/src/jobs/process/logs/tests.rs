//! Tests for log-file creation and the log-directory seam.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;

use super::{
    create_log_file_in, create_log_path_in, require, slugify_target, test_log_dir, unusable_log_dir,
};
use crate::AppErrorKind;

fn mode_of(path: &std::path::Path) -> u32 {
    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn create_log_path_in_creates_a_named_file_in_the_directory() {
    let root = tempfile::tempdir().unwrap();
    let log_dir = test_log_dir(root.path());
    let dir = require(&log_dir).expect("usable log dir");

    let path = create_log_path_in(dir, "game", "quest").expect("log file");

    assert_eq!(path.parent(), Some(dir.path()));
    assert!(path.is_file(), "the log file exists and is empty");
    assert!(path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("game-quest-"));
}

#[test]
fn create_log_path_in_never_truncates_an_existing_same_millisecond_file() {
    let root = tempfile::tempdir().unwrap();
    let log_dir = test_log_dir(root.path());
    let dir = require(&log_dir).unwrap();
    // Two sessions in the same millisecond resolve to the same stem; the
    // second call must not open (and truncate) the first file.
    let first = create_log_path_in(dir, "game", "twin").expect("first log file");
    std::fs::write(&first, b"first-session-bytes").unwrap();

    let second = create_log_path_in(dir, "game", "twin").expect("second log file");

    assert_ne!(first, second, "the second session gets its own file");
    assert_eq!(
        std::fs::read(&first).unwrap(),
        b"first-session-bytes",
        "the first session's log is never truncated"
    );
    assert!(second.is_file());
}

#[test]
fn create_log_file_in_is_owner_only_and_writes_through_the_handle() {
    let root = tempfile::tempdir().unwrap();
    let log_dir = test_log_dir(root.path());
    let dir = require(&log_dir).unwrap();

    let (path, mut file) = create_log_file_in(dir, "run", "tool").expect("log file");

    assert_eq!(path.parent(), Some(dir.path()));
    let mode = mode_of(&path);
    assert_eq!(mode, 0o600, "log file mode is {mode:o}");

    // Child stdio is attached to clones of the returned handle.
    let mut clone = file.try_clone().expect("clone handle");
    file.write_all(b"first\n").unwrap();
    clone.write_all(b"second\n").unwrap();
    drop((file, clone));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
}

#[test]
fn require_maps_an_unusable_directory_to_a_path_free_io_error() {
    let log_dir = unusable_log_dir();

    let error = require(&log_dir).expect_err("no log directory");

    assert_eq!(error.kind(), AppErrorKind::Io);
    assert_eq!(error.message(), "no private log directory available");
}

#[test]
fn slugify_target_lowercases_and_trims_separators() {
    assert_eq!(slugify_target("  My Game: 2!! ", "fallback"), "my-game--2");
    assert_eq!(slugify_target("***", "fallback"), "fallback");
}
