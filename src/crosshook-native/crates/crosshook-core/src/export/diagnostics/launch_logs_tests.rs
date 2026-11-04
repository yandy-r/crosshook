use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nix::sys::stat::Mode;
use tempfile::{tempdir, TempDir};

use super::*;

fn euid() -> u32 {
    nix::unistd::geteuid().as_raw()
}

/// Private (0700) launch log directory inside a fresh temp root.
fn private_log_dir() -> (TempDir, PathBuf) {
    let root = tempdir().unwrap();
    let dir = root.path().join("crosshook-logs");
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    (root, dir)
}

fn names(logs: &[(String, Vec<u8>)]) -> Vec<&str> {
    logs.iter().map(|(name, _)| name.as_str()).collect()
}

/// Runs `collect` on another thread so a blocking open fails the test
/// instead of hanging it.
fn collect_with_timeout(dir: PathBuf, owner: u32) -> Vec<(String, Vec<u8>)> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(collect_launch_logs_from(&dir, owner));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("launch log collection blocked")
}

#[test]
fn caps_at_max_files() {
    // More than 2 * MAX_LAUNCH_LOG_FILES, so the rolling trim runs mid-scan.
    let (_root, dir) = private_log_dir();
    let base = std::time::SystemTime::now() - Duration::from_secs(3600);
    for i in 0..25u64 {
        let path = dir.join(format!("game-{i:02}.log"));
        fs::write(&path, format!("log {i}")).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(base + Duration::from_secs(i))
            .unwrap();
    }

    let logs = collect_launch_logs_from(&dir, euid());

    let expected: Vec<String> = (15..25).rev().map(|i| format!("game-{i:02}.log")).collect();
    assert_eq!(names(&logs), expected);
}

#[test]
fn returns_most_recent_first() {
    let (_root, dir) = private_log_dir();
    fs::write(dir.join("old.log"), "old").unwrap();
    thread::sleep(Duration::from_millis(20));
    fs::write(dir.join("new.log"), "new").unwrap();
    fs::write(dir.join("notes.txt"), "not a log").unwrap();

    let logs = collect_launch_logs_from(&dir, euid());

    assert_eq!(names(&logs), ["new.log", "old.log"]);
    assert_eq!(logs[0].1, b"new");
}

#[test]
fn skips_symlinked_log_file() {
    let (root, dir) = private_log_dir();
    let outside = root.path().join("outside.txt");
    fs::write(&outside, "outside contents").unwrap();
    symlink(&outside, dir.join("a.log")).unwrap();
    fs::write(dir.join("b.log"), "real log").unwrap();

    let logs = collect_launch_logs_from(&dir, euid());

    assert_eq!(names(&logs), ["b.log"]);
}

#[test]
fn skips_fifo_without_blocking() {
    let (_root, dir) = private_log_dir();
    nix::unistd::mkfifo(&dir.join("x.log"), Mode::from_bits_truncate(0o600)).unwrap();
    fs::write(dir.join("b.log"), "real log").unwrap();

    let logs = collect_with_timeout(dir, euid());

    assert_eq!(names(&logs), ["b.log"]);
}

/// A directory is dropped either by the regular-file check or by the failed
/// read; the FIFO test is the one that pins the regular-file check.
#[test]
fn skips_directory_named_like_a_log() {
    let (_root, dir) = private_log_dir();
    fs::create_dir(dir.join("x.log")).unwrap();
    fs::write(dir.join("b.log"), "real log").unwrap();

    let logs = collect_launch_logs_from(&dir, euid());

    assert_eq!(names(&logs), ["b.log"]);
}

#[test]
fn skips_group_or_other_writable_dir() {
    for mode in [0o720, 0o702, 0o777] {
        let (_root, dir) = private_log_dir();
        fs::write(dir.join("a.log"), "log").unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).unwrap();

        assert!(
            collect_launch_logs_from(&dir, euid()).is_empty(),
            "mode {mode:o}"
        );
    }
}

#[test]
fn skips_dir_owned_by_another_user() {
    let (_root, dir) = private_log_dir();
    fs::write(dir.join("a.log"), "log").unwrap();

    assert!(collect_launch_logs_from(&dir, euid().wrapping_add(1)).is_empty());
}

#[test]
fn skips_symlinked_dir() {
    let (root, dir) = private_log_dir();
    fs::write(dir.join("a.log"), "log").unwrap();
    let link = root.path().join("linked-logs");
    symlink(&dir, &link).unwrap();

    assert!(collect_launch_logs_from(&link, euid()).is_empty());
}

#[test]
fn missing_dir_yields_nothing() {
    let root = tempdir().unwrap();
    assert!(collect_launch_logs_from(&root.path().join("absent"), euid()).is_empty());
}

#[test]
fn collects_cli_logs_under_their_own_bundle_dir() {
    let (_app_root, app_dir) = private_log_dir();
    let (_cli_root, cli_dir) = private_log_dir();
    fs::write(app_dir.join("game.log"), "app").unwrap();
    fs::write(cli_dir.join("cli-game.log"), "cli").unwrap();

    let logs = collect_launch_logs_with(&app_dir, Some(&cli_dir), euid());

    assert_eq!(
        names(&logs),
        ["logs/launch/game.log", "logs/launch-cli/cli-game.log"]
    );
}

#[test]
fn each_dir_keeps_its_own_cap() {
    let (_app_root, app_dir) = private_log_dir();
    let (_cli_root, cli_dir) = private_log_dir();
    for i in 0..12 {
        fs::write(app_dir.join(format!("app-{i:02}.log")), "app").unwrap();
        fs::write(cli_dir.join(format!("cli-{i:02}.log")), "cli").unwrap();
    }

    let logs = collect_launch_logs_with(&app_dir, Some(&cli_dir), euid());

    assert_eq!(logs.len(), 2 * MAX_LAUNCH_LOG_FILES);
}

#[test]
fn cli_dir_resolving_to_app_dir_is_collected_once() {
    let (root, app_dir) = private_log_dir();
    fs::write(app_dir.join("game.log"), "app").unwrap();
    // Same directory reached through a symlinked parent.
    let parent_link = root.path().join("runtime");
    symlink(root.path(), &parent_link).unwrap();
    let cli_dir = parent_link.join("crosshook-logs");

    let logs = collect_launch_logs_with(&app_dir, Some(&cli_dir), euid());

    assert_eq!(names(&logs), ["logs/launch/game.log"]);
}

#[test]
fn cli_launch_log_dir_requires_absolute_runtime_dir() {
    assert_eq!(cli_launch_log_dir(None), None);
    assert_eq!(cli_launch_log_dir(Some(OsString::new())), None);
    assert_eq!(cli_launch_log_dir(Some("relative".into())), None);
    assert_eq!(
        cli_launch_log_dir(Some("/run/user/1000".into())),
        Some(PathBuf::from("/run/user/1000/crosshook-logs"))
    );
}
