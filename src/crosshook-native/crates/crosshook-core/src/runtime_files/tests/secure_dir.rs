//! Tests for the `secure_dir` module.

use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::Path;

use nix::unistd::geteuid;
use tempfile::TempDir;

use super::super::secure_dir::{open_private_dir_owned_by, open_private_dir_with_owners};
use super::super::{open_private_dir, CandidateRoot, LogDirCandidate, RuntimeDirError};

fn candidate(root: &Path, relative: &[&'static str], kind: CandidateRoot) -> LogDirCandidate {
    LogDirCandidate {
        root: root.to_path_buf(),
        relative: relative.to_vec(),
        kind,
    }
}

fn runtime(root: &Path) -> LogDirCandidate {
    candidate(root, &["crosshook", "logs"], CandidateRoot::Runtime)
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn secure_root() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    set_mode(root.path(), 0o700);
    root
}

#[test]
fn creates_both_components_private() {
    let root = secure_root();
    let dir = open_private_dir(&runtime(root.path())).unwrap();
    assert_eq!(dir.path(), root.path().join("crosshook/logs"));
    assert_eq!(mode_of(&root.path().join("crosshook")), 0o700);
    assert_eq!(mode_of(&root.path().join("crosshook/logs")), 0o700);
}

#[test]
fn reopening_an_existing_directory_succeeds() {
    let root = secure_root();
    open_private_dir(&runtime(root.path())).unwrap();
    open_private_dir(&runtime(root.path())).unwrap();
}

#[test]
fn symlink_at_the_first_component_is_refused_and_target_untouched() {
    let root = secure_root();
    let target = tempfile::tempdir().unwrap();
    set_mode(target.path(), 0o755);
    symlink(target.path(), root.path().join("crosshook")).unwrap();

    let error = open_private_dir(&runtime(root.path())).unwrap_err();

    assert!(matches!(error, RuntimeDirError::NotADirectory), "{error:?}");
    assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    assert_eq!(mode_of(target.path()), 0o755);
}

#[test]
fn symlink_at_the_leaf_is_refused_and_target_untouched() {
    let root = secure_root();
    let target = tempfile::tempdir().unwrap();
    set_mode(target.path(), 0o755);
    fs::create_dir(root.path().join("crosshook")).unwrap();
    symlink(target.path(), root.path().join("crosshook/logs")).unwrap();

    let error = open_private_dir(&runtime(root.path())).unwrap_err();

    assert!(matches!(error, RuntimeDirError::NotADirectory), "{error:?}");
    assert_eq!(mode_of(target.path()), 0o755);
}

#[test]
fn existing_regular_file_component_is_not_a_directory() {
    let root = secure_root();
    fs::write(root.path().join("crosshook"), b"x").unwrap();
    let error = open_private_dir(&runtime(root.path())).unwrap_err();
    assert!(matches!(error, RuntimeDirError::NotADirectory), "{error:?}");
}

#[test]
fn foreign_owned_root_is_not_usable() {
    // With the single-owner seam the root is checked against the same owner,
    // so a foreign owner is rejected at the root before any component is made.
    let root = secure_root();
    let foreign = geteuid().as_raw().wrapping_add(1);
    let error = open_private_dir_owned_by(&runtime(root.path()), foreign).unwrap_err();
    assert!(matches!(error, RuntimeDirError::NoUsableDir), "{error:?}");
    assert!(!root.path().join("crosshook").exists());
}

#[test]
fn foreign_owned_component_is_refused() {
    // Real owner for the root, a different expected owner for the components:
    // the freshly created (current-user) component then looks foreign.
    let root = secure_root();
    let me = geteuid().as_raw();
    let error =
        open_private_dir_with_owners(&runtime(root.path()), me, me.wrapping_add(1)).unwrap_err();
    assert!(matches!(error, RuntimeDirError::ForeignOwner), "{error:?}");
}

#[test]
fn sticky_world_writable_leaf_is_tightened() {
    let root = secure_root();
    let leaf = root.path().join("crosshook/logs");
    fs::create_dir_all(&leaf).unwrap();
    set_mode(&leaf, 0o1777);

    open_private_dir(&runtime(root.path())).unwrap();

    assert_eq!(mode_of(&leaf), 0o700);
    assert_eq!(fs::metadata(&leaf).unwrap().uid(), geteuid().as_raw());
}

#[test]
fn group_other_writable_root_is_not_usable() {
    let root = secure_root();
    set_mode(root.path(), 0o777);
    let error = open_private_dir(&runtime(root.path())).unwrap_err();
    assert!(matches!(error, RuntimeDirError::NoUsableDir), "{error:?}");
    assert_eq!(mode_of(root.path()), 0o777, "the root is never chmodded");
    assert!(!root.path().join("crosshook").exists());
}

#[test]
fn missing_runtime_root_is_not_created() {
    let root = secure_root();
    let missing = root.path().join("absent");
    let error = open_private_dir(&runtime(&missing)).unwrap_err();
    assert!(matches!(error, RuntimeDirError::NoUsableDir), "{error:?}");
    assert!(!missing.exists());
}

#[test]
fn missing_state_root_is_created_private() {
    let root = secure_root();
    let state = root.path().join("state/nested");
    let candidate = candidate(&state, &["crosshook", "logs"], CandidateRoot::State);

    open_private_dir(&candidate).unwrap();

    assert_eq!(mode_of(&state), 0o700);
    assert!(state.join("crosshook/logs").is_dir());
}

#[test]
fn group_writable_state_root_is_usable_and_leaf_private() {
    let root = secure_root();
    let state = root.path().join("state");
    fs::create_dir(&state).unwrap();
    set_mode(&state, 0o775);
    let candidate = candidate(&state, &["crosshook", "logs"], CandidateRoot::State);

    open_private_dir(&candidate).unwrap();

    assert_eq!(mode_of(&state), 0o775, "the root is never chmodded");
    assert_eq!(mode_of(&state.join("crosshook")), 0o700);
    assert_eq!(mode_of(&state.join("crosshook/logs")), 0o700);
}

#[test]
fn world_writable_state_root_is_not_usable() {
    let root = secure_root();
    let state = root.path().join("state");
    fs::create_dir(&state).unwrap();
    set_mode(&state, 0o777);
    let candidate = candidate(&state, &["crosshook", "logs"], CandidateRoot::State);

    let error = open_private_dir(&candidate).unwrap_err();
    assert!(matches!(error, RuntimeDirError::NoUsableDir), "{error:?}");
}
