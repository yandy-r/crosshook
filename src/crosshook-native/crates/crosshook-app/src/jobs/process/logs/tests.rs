//! Tests for log-file creation and the log-directory hardening.

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

use super::{create_log_file_in, create_log_path_in, create_unique_log, secure_log_dir_owned_by};

fn mode_of(path: &std::path::Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn create_log_path_in_creates_the_directory_and_a_unique_file() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("deep/logs");

    let path = create_log_path_in(nested.clone(), "game", "quest")
        .expect("log file in the given directory");

    assert_eq!(path.parent(), Some(nested.as_path()));
    assert!(path.is_file(), "the log file exists and is empty");
    assert!(path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("game-quest-"));
}

#[test]
fn create_log_path_in_never_truncates_an_existing_same_millisecond_file() {
    let dir = tempfile::tempdir().unwrap();
    // Two sessions in the same millisecond resolve to the same stem; the
    // second call must not open (and truncate) the first file.
    let first =
        create_log_path_in(dir.path().to_path_buf(), "game", "twin").expect("first log file");
    fs::write(&first, b"first-session-bytes").unwrap();

    let second =
        create_log_path_in(dir.path().to_path_buf(), "game", "twin").expect("second log file");

    assert_ne!(first, second, "the second session gets its own file");
    assert_eq!(
        fs::read(&first).unwrap(),
        b"first-session-bytes",
        "the first session's log is never truncated"
    );
    assert!(second.is_file());
}

#[test]
fn create_log_file_in_is_owner_only_and_writes_through_the_handle() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut file) =
        create_log_file_in(dir.path().to_path_buf(), "run", "tool").expect("log file");

    assert_eq!(path.parent(), Some(dir.path()));
    let mode = mode_of(&path);
    assert_eq!(mode, 0o600, "log file mode is {mode:o}");

    // Child stdio is attached to clones of the returned handle.
    let mut clone = file.try_clone().expect("clone handle");
    file.write_all(b"first\n").unwrap();
    clone.write_all(b"second\n").unwrap();
    drop((file, clone));
    assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
}

#[test]
fn a_symlink_planted_at_the_next_log_name_is_refused_and_its_target_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("victim.txt");
    fs::write(&target, b"precious").unwrap();
    let planted = dir.path().join("run-tool-1.log");
    symlink(&target, &planted).unwrap();

    let (path, mut file) = create_unique_log(dir.path(), "run-tool-1").expect("log file");
    file.write_all(b"log output\n").unwrap();
    drop(file);

    assert_eq!(
        path,
        dir.path().join("run-tool-1-1.log"),
        "the next free name"
    );
    assert_eq!(fs::read(&target).unwrap(), b"precious", "target untouched");
    assert!(fs::symlink_metadata(&planted)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&path).unwrap(), "log output\n");
}

#[test]
fn a_new_log_directory_is_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");

    create_log_file_in(logs.clone(), "run", "tool").expect("log file");

    assert_eq!(mode_of(&logs), 0o700);
}

#[test]
fn an_existing_shared_log_directory_is_tightened_to_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).unwrap();
    fs::set_permissions(&logs, fs::Permissions::from_mode(0o1777)).unwrap();

    create_log_file_in(logs.clone(), "run", "tool").expect("log file");

    assert_eq!(mode_of(&logs), 0o700);
}

#[test]
fn a_symlinked_log_directory_is_refused_without_creating_anything() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("elsewhere");
    fs::create_dir(&real).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o755)).unwrap();
    let logs = dir.path().join("logs");
    symlink(&real, &logs).unwrap();

    let error = create_log_file_in(logs, "run", "tool").expect_err("symlinked dir");

    assert!(error.contains("symlink"), "{error}");
    assert_eq!(fs::read_dir(&real).unwrap().count(), 0, "nothing created");
    assert_eq!(mode_of(&real), 0o755, "the target is not chmodded");
}

#[test]
fn a_log_directory_owned_by_another_user_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).unwrap();
    fs::set_permissions(&logs, fs::Permissions::from_mode(0o755)).unwrap();
    let other = fs::metadata(&logs).unwrap().uid().wrapping_add(1);

    let error = secure_log_dir_owned_by(&logs, other).expect_err("foreign owner");

    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("another user"), "{error}");
    assert_eq!(mode_of(&logs), 0o755, "a foreign directory is not chmodded");
}
