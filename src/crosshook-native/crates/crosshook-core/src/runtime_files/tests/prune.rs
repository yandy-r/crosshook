//! Tests for the `prune` module.

use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::{Duration, SystemTime};

use nix::sys::stat::Mode;
use tempfile::TempDir;

use super::super::{
    open_private_dir, prune_stale, CandidateRoot, LogDirCandidate, PrivateDir, RetentionPolicy,
};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

fn private_dir(root: &TempDir) -> PrivateDir {
    open_private_dir(&LogDirCandidate {
        root: root.path().to_path_buf(),
        relative: vec!["logs"],
        kind: CandidateRoot::State,
    })
    .unwrap()
}

fn policy(max_age_days: u32, max_files: usize) -> RetentionPolicy {
    RetentionPolicy {
        max_age: DAY * max_age_days,
        max_files,
    }
}

fn file_aged(dir: &Path, name: &str, now: SystemTime, age: Duration) {
    let file = File::create(dir.join(name)).unwrap();
    file.set_modified(now - age).unwrap();
}

#[test]
fn default_policy_uses_the_documented_limits() {
    let policy = RetentionPolicy::default();
    assert_eq!(policy.max_age, DAY * 7);
    assert_eq!(policy.max_files, 200);
}

#[test]
fn removes_files_older_than_the_max_age() {
    let root = TempDir::new().unwrap();
    let dir = private_dir(&root);
    let logs = dir.path();
    let now = SystemTime::now();
    file_aged(logs, "old.log", now, DAY * 10);
    file_aged(logs, "old.gamescope.pid", now, DAY * 8);
    file_aged(logs, "fresh.log", now, DAY);

    assert_eq!(prune_stale(&dir, &policy(7, 100), now), 2);

    assert!(!logs.join("old.log").exists());
    assert!(!logs.join("old.gamescope.pid").exists());
    assert!(logs.join("fresh.log").exists());
}

#[test]
fn keeps_only_the_newest_files_up_to_the_limit() {
    let root = TempDir::new().unwrap();
    let dir = private_dir(&root);
    let logs = dir.path();
    let now = SystemTime::now();
    for (name, hours) in [
        ("a.log", 1),
        ("b.log", 2),
        ("c.log", 3),
        ("d.gamescope.pid", 4),
    ] {
        file_aged(logs, name, now, Duration::from_secs(hours * 3600));
    }

    assert_eq!(prune_stale(&dir, &policy(7, 2), now), 2);

    assert!(logs.join("a.log").exists());
    assert!(logs.join("b.log").exists());
    assert!(!logs.join("c.log").exists());
    assert!(!logs.join("d.gamescope.pid").exists());
}

#[test]
fn leaves_unrelated_and_unsafe_entries_untouched() {
    let root = TempDir::new().unwrap();
    let dir = private_dir(&root);
    let logs = dir.path();
    let now = SystemTime::now();
    let ancient = DAY * 100;

    file_aged(logs, "instance.lock", now, ancient);
    file_aged(logs, "x.txt", now, ancient);
    let victim = root.path().join("victim.log");
    fs::write(&victim, "keep").unwrap();
    symlink(&victim, logs.join("y.log")).unwrap();
    nix::unistd::mkfifo(&logs.join("z.log"), Mode::from_bits_truncate(0o600)).unwrap();
    fs::create_dir(logs.join("sub.log")).unwrap();

    assert_eq!(prune_stale(&dir, &policy(7, 0), now), 0);

    for name in ["instance.lock", "x.txt", "y.log", "z.log", "sub.log"] {
        assert!(fs::symlink_metadata(logs.join(name)).is_ok(), "{name}");
    }
    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
}

#[test]
fn files_with_future_mtimes_are_not_aged_out() {
    let root = TempDir::new().unwrap();
    let dir = private_dir(&root);
    let now = SystemTime::now();
    let file = File::create(dir.path().join("future.log")).unwrap();
    file.set_modified(now + DAY).unwrap();

    assert_eq!(prune_stale(&dir, &policy(7, 10), now), 0);
    assert!(dir.path().join("future.log").exists());
}

#[test]
fn an_empty_directory_prunes_nothing() {
    let root = TempDir::new().unwrap();
    let dir = private_dir(&root);
    assert_eq!(
        prune_stale(&dir, &RetentionPolicy::default(), SystemTime::now()),
        0
    );
}

#[test]
fn pruning_twice_through_one_handle_sees_every_entry() {
    let root = TempDir::new().unwrap();
    let dir = private_dir(&root);
    let logs = dir.path();
    let now = SystemTime::now();
    file_aged(logs, "first.log", now, DAY * 10);
    assert_eq!(prune_stale(&dir, &policy(7, 100), now), 1);

    file_aged(logs, "second.log", now, DAY * 10);
    assert_eq!(prune_stale(&dir, &policy(7, 100), now), 1);
    assert!(!logs.join("second.log").exists());
}
