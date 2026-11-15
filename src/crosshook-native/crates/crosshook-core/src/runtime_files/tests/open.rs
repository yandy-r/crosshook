//! Tests for the `open` module.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nix::sys::stat::Mode;
use tempfile::TempDir;

use super::super::{
    create_unique_log_file, open_private_append, open_private_dir, open_private_read,
    CandidateRoot, LogDirCandidate, PrivateDir, RuntimeDirError,
};

fn private_dir(root: &Path) -> PrivateDir {
    open_private_dir(&LogDirCandidate {
        root: root.to_path_buf(),
        relative: vec!["logs"],
        kind: CandidateRoot::State,
    })
    .unwrap()
}

fn setup() -> (TempDir, PrivateDir) {
    let root = tempfile::tempdir().unwrap();
    let dir = private_dir(root.path());
    (root, dir)
}

#[test]
fn creates_the_bare_name_first_with_mode_0600() {
    let (_root, dir) = setup();
    let (path, _file) = create_unique_log_file(&dir, "run-tool-1").unwrap();
    assert_eq!(path, dir.path().join("run-tool-1.log"));
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode, 0o600);
}

#[test]
fn second_call_gets_a_distinct_file_and_does_not_truncate_the_first() {
    let (_root, dir) = setup();
    let (first, mut first_file) = create_unique_log_file(&dir, "run").unwrap();
    first_file.write_all(b"first").unwrap();
    let (second, _second_file) = create_unique_log_file(&dir, "run").unwrap();
    assert_eq!(first, dir.path().join("run.log"));
    assert_eq!(second, dir.path().join("run-1.log"));
    assert_eq!(fs::read(&first).unwrap(), b"first");
}

#[test]
fn planted_symlink_is_skipped_and_the_victim_is_untouched() {
    let (root, dir) = setup();
    let victim = root.path().join("victim.txt");
    fs::write(&victim, b"keep").unwrap();
    symlink(&victim, dir.path().join("run.log")).unwrap();

    let (path, mut file) = create_unique_log_file(&dir, "run").unwrap();
    file.write_all(b"new").unwrap();

    assert_eq!(path, dir.path().join("run-1.log"));
    assert_eq!(fs::read(&victim).unwrap(), b"keep");
}

#[test]
fn dangling_symlink_is_skipped_without_creating_its_target() {
    let (root, dir) = setup();
    let target = root.path().join("absent.txt");
    symlink(&target, dir.path().join("run.log")).unwrap();

    let (path, _file) = create_unique_log_file(&dir, "run").unwrap();

    assert_eq!(path, dir.path().join("run-1.log"));
    assert!(!target.exists());
}

#[test]
fn running_out_of_names_returns_names_exhausted() {
    let (_root, dir) = setup();
    fs::File::create(dir.path().join("run.log")).unwrap();
    for suffix in 1..=999 {
        fs::File::create(dir.path().join(format!("run-{suffix}.log"))).unwrap();
    }
    assert!(matches!(
        create_unique_log_file(&dir, "run"),
        Err(RuntimeDirError::NamesExhausted)
    ));
}

#[test]
fn append_adds_to_an_existing_file_and_never_creates() {
    let (_root, dir) = setup();
    let path = dir.path().join("a.log");
    fs::write(&path, b"one").unwrap();

    let mut file = open_private_append(&path).unwrap();
    file.write_all(b"two").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"onetwo");

    let missing = dir.path().join("missing.log");
    assert!(open_private_append(&missing).is_err());
    assert!(!missing.exists());
}

#[test]
fn append_on_a_symlink_is_refused_and_the_target_untouched() {
    let (root, dir) = setup();
    let victim = root.path().join("victim.txt");
    fs::write(&victim, b"keep").unwrap();
    let link = dir.path().join("a.log");
    symlink(&victim, &link).unwrap();

    assert!(matches!(
        open_private_append(&link),
        Err(RuntimeDirError::UnsafeFile)
    ));
    assert_eq!(fs::read(&victim).unwrap(), b"keep");
}

#[test]
fn read_opens_a_regular_file() {
    let (_root, dir) = setup();
    let path = dir.path().join("a.log");
    fs::write(&path, b"data").unwrap();
    let mut file = open_private_read(&path).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).unwrap();
    assert_eq!(text, "data");
}

#[test]
fn read_on_a_symlink_is_refused_and_the_target_untouched() {
    let (root, dir) = setup();
    let victim = root.path().join("victim.txt");
    fs::write(&victim, b"keep").unwrap();
    let link = dir.path().join("a.log");
    symlink(&victim, &link).unwrap();

    assert!(open_private_read(&link).is_err());
    assert_eq!(fs::read(&victim).unwrap(), b"keep");
}

#[test]
fn read_on_a_directory_is_unsafe_file() {
    let (_root, dir) = setup();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    assert!(matches!(
        open_private_read(&sub),
        Err(RuntimeDirError::UnsafeFile)
    ));
}

#[test]
fn read_on_a_fifo_does_not_block_and_is_unsafe_file() {
    let (_root, dir) = setup();
    let fifo = dir.path().join("x.log");
    nix::unistd::mkfifo(&fifo, Mode::from_bits_truncate(0o600)).unwrap();

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(open_private_read(&fifo).map(|_| ()));
    });
    let result = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("opening a FIFO blocked");
    assert!(matches!(result, Err(RuntimeDirError::UnsafeFile)));
}

#[test]
fn append_on_a_fifo_does_not_block_and_fails() {
    let (_root, dir) = setup();
    let fifo = dir.path().join("x.log");
    nix::unistd::mkfifo(&fifo, Mode::from_bits_truncate(0o600)).unwrap();

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(open_private_append(&fifo).map(|_| ()));
    });
    let result = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("appending to a FIFO blocked");
    assert!(result.is_err());
}

#[test]
fn unsafe_stems_are_refused_without_creating_anything() {
    let (root, dir) = setup();
    for stem in ["", ".hidden", "..", "../escape", "a/b", "nul\0byte"] {
        let error = create_unique_log_file(&dir, stem).unwrap_err();
        assert!(
            matches!(error, RuntimeDirError::UnsafeFile),
            "stem {stem:?}"
        );
    }
    assert!(!root.path().join("escape.log").exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}
