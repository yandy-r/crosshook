//! Throwaway cleanup: the namespace guard, symlinks never followed, bounded
//! retries with a sweep, and the job holding its key while cleanup runs.

use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{key_of, Fixture, Run};
use crate::services::process::JobOwner;
use crate::services::run_executable::cleanup::{
    cleanup_throwaway_prefix, cleanup_with, CleanupOutcome,
};
use crate::services::run_executable::{RunCompletion, RunExecutableService};

const NO_SETTLE: [Duration; 2] = [Duration::ZERO, Duration::ZERO];

/// A `_run-adhoc` root with one throwaway prefix holding a file.
struct Tree {
    _dir: tempfile::TempDir,
    base: PathBuf,
    root: PathBuf,
    prefix: PathBuf,
}

fn tree() -> Tree {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let root = base.join("_run-adhoc");
    let prefix = root.join("setup-0a1b2c3d4e5f");
    std::fs::create_dir_all(prefix.join("drive_c")).unwrap();
    std::fs::write(prefix.join("drive_c/file.txt"), b"x").unwrap();
    Tree {
        _dir: dir,
        base,
        root,
        prefix,
    }
}

/// A sweep that must never run.
fn no_sweep(path: &Path) {
    panic!("unexpected sweep of {}", path.display());
}

#[test]
fn removes_a_throwaway_prefix_without_a_sweep() {
    let tree = tree();
    assert_eq!(
        cleanup_with(&tree.prefix, &tree.root, NO_SETTLE, no_sweep),
        CleanupOutcome::Removed
    );
    assert!(!tree.prefix.exists());
    assert!(tree.root.is_dir(), "the namespace root itself is kept");
}

#[test]
fn refuses_every_path_outside_the_namespace() {
    let tree = tree();
    let outside = tree.base.join("real-game");
    std::fs::create_dir(&outside).unwrap();
    let candidates = [
        tree.root.clone(),
        tree.base.clone(),
        outside.clone(),
        tree.prefix.join("drive_c"),
        tree.root.join("..").join("real-game"),
        tree.prefix.join("..").join("setup-0a1b2c3d4e5f"),
    ];
    for candidate in candidates {
        assert_eq!(
            cleanup_with(&candidate, &tree.root, NO_SETTLE, no_sweep),
            CleanupOutcome::NotThrowaway,
            "{}",
            candidate.display()
        );
    }
    assert!(outside.is_dir());
    assert!(tree.prefix.join("drive_c/file.txt").is_file());
}

#[test]
fn a_missing_prefix_is_reported_without_touching_anything() {
    let tree = tree();
    let missing = tree.root.join("gone-123");
    assert_eq!(
        cleanup_with(&missing, &tree.root, NO_SETTLE, no_sweep),
        CleanupOutcome::Missing
    );
}

#[test]
fn a_symlinked_prefix_is_unlinked_and_its_target_kept() {
    let tree = tree();
    let target = tree.base.join("precious");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), b"keep").unwrap();
    let link = tree.root.join("setup-link");
    symlink(&target, &link).unwrap();

    assert_eq!(
        cleanup_with(&link, &tree.root, NO_SETTLE, no_sweep),
        CleanupOutcome::Removed
    );
    assert!(std::fs::symlink_metadata(&link).is_err(), "link removed");
    assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn symlinks_inside_the_prefix_are_not_followed() {
    let tree = tree();
    let target = tree.base.join("precious");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), b"keep").unwrap();
    symlink(&target, tree.prefix.join("drive_c/escape")).unwrap();

    assert_eq!(
        cleanup_throwaway_prefix(&tree.prefix, &tree.root),
        CleanupOutcome::Removed
    );
    assert!(!tree.prefix.exists());
    assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"keep");
}

/// Makes `dir`'s entries undeletable (no write permission on `dir`);
/// returns `false` when permissions are not enforced (running as root).
fn lock_dir(dir: &Path) -> bool {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let probe = dir.join(".probe");
    if std::fs::write(&probe, b"").is_ok() {
        std::fs::remove_file(probe).unwrap();
        unlock_dir(dir);
        return false;
    }
    true
}

fn unlock_dir(dir: &Path) {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_failed_removal_sweeps_and_retries_until_it_succeeds() {
    let tree = tree();
    let locked = tree.prefix.join("drive_c");
    if !lock_dir(&locked) {
        return;
    }
    let mut sweeps = Vec::new();
    let outcome = cleanup_with(&tree.prefix, &tree.root, NO_SETTLE, |path| {
        sweeps.push(path.to_path_buf());
        // The "lingering process" goes away on the second sweep.
        if sweeps.len() == 2 {
            unlock_dir(&locked);
        }
    });
    assert_eq!(outcome, CleanupOutcome::Removed);
    assert_eq!(sweeps, vec![tree.prefix.clone(), tree.prefix.clone()]);
    assert!(!tree.prefix.exists());
}

#[test]
fn retries_are_bounded_and_end_in_failed() {
    let tree = tree();
    let locked = tree.prefix.join("drive_c");
    if !lock_dir(&locked) {
        return;
    }
    let mut sweeps = 0;
    let outcome = cleanup_with(&tree.prefix, &tree.root, NO_SETTLE, |path| {
        assert_eq!(path, tree.prefix);
        sweeps += 1;
    });
    unlock_dir(&locked);
    assert_eq!(outcome, CleanupOutcome::Failed);
    assert_eq!(sweeps, 2, "exactly two bounded retries");
    assert!(locked.join("file.txt").is_file());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_key_is_held_while_cleanup_runs_and_released_after() {
    let fixture = Fixture::new();
    fixture.install_proton("echo done");
    // The cleanup seam reports its arguments, then blocks until released,
    // then runs the real cleanup with those arguments.
    let (entered_tx, entered_rx) = mpsc::channel::<(PathBuf, PathBuf)>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let service = RunExecutableService::with_dirs(
        fixture.jobs.clone(),
        fixture.log_dir.clone(),
        fixture.adhoc_root.clone(),
    )
    .with_cleanup(move |prefix, root| {
        entered_tx
            .send((prefix.to_path_buf(), root.to_path_buf()))
            .unwrap();
        release_rx.lock().unwrap().recv().unwrap();
        cleanup_throwaway_prefix(prefix, root)
    });
    let (response, run) = service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .unwrap();
    let mut run = Run::new(run);
    let prefix = response.result.resolved_prefix_path.clone();
    let key = key_of(&prefix);
    assert_eq!(run.next_line().await, format!("prefix={prefix}"));
    assert_eq!(run.next_line().await, "done");

    let (seen_prefix, seen_root) = tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();
    assert_eq!(seen_prefix, PathBuf::from(&prefix));
    assert_eq!(seen_root, fixture.adhoc_root);
    // The child has exited, but the job is inside cleanup: key still held,
    // prefix still present, job still running.
    assert!(fixture.jobs.is_key_held(&key));
    assert!(Path::new(&prefix).is_dir());
    assert_eq!(fixture.jobs.job_counts(), (1, 0));

    release_tx.send(()).unwrap();
    let (lines, completion) = run.drain().await;
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(
        completion,
        Some(RunCompletion {
            code: Some(0),
            signal: None
        })
    );
    assert!(!fixture.jobs.is_key_held(&key));
    assert!(!Path::new(&prefix).exists());
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}
