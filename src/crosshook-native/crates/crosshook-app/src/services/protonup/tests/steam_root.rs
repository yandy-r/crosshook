//! Direct tests of the Steam root selection used by uninstall.

use std::fs;
use std::path::{Path, PathBuf};

use super::super::uninstall::select_steam_root;

fn dir(base: &Path, name: &str) -> PathBuf {
    let path = base.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn caller_path_equal_to_a_trusted_root_selects_that_root() {
    let tmp = tempfile::tempdir().unwrap();
    let first = dir(tmp.path(), "First");
    let second = dir(tmp.path(), "Second");
    let trusted = vec![first.clone(), second.clone()];

    assert_eq!(
        select_steam_root(&trusted, Some(&text(&second))),
        Some(second)
    );
}

#[cfg(unix)]
#[test]
fn alias_spelling_of_a_trusted_root_returns_the_trusted_spelling() {
    let tmp = tempfile::tempdir().unwrap();
    let first = dir(tmp.path(), "First");
    let second = dir(tmp.path(), "Second");
    let alias = tmp.path().join("SecondAlias");
    std::os::unix::fs::symlink(&second, &alias).unwrap();
    let trusted = vec![first, second.clone()];

    assert_eq!(
        select_steam_root(&trusted, Some(&text(&alias))),
        Some(second)
    );
}

#[test]
fn untrusted_caller_path_falls_back_to_the_first_trusted_root() {
    let tmp = tempfile::tempdir().unwrap();
    let first = dir(tmp.path(), "First");
    let other = dir(tmp.path(), "Other");
    let trusted = vec![first.clone()];

    assert_eq!(
        select_steam_root(&trusted, Some(&text(&other))),
        Some(first)
    );
}

#[test]
fn missing_caller_path_falls_back_to_the_first_trusted_root() {
    let tmp = tempfile::tempdir().unwrap();
    let first = dir(tmp.path(), "First");
    let ghost = tmp.path().join("does-not-exist");
    let trusted = vec![first.clone()];

    assert_eq!(
        select_steam_root(&trusted, Some(&text(&ghost))),
        Some(first)
    );
}

#[test]
fn blank_and_absent_caller_paths_fall_back_to_the_first_trusted_root() {
    let tmp = tempfile::tempdir().unwrap();
    let first = dir(tmp.path(), "First");
    let second = dir(tmp.path(), "Second");
    let trusted = vec![first.clone(), second];

    for caller in [None, Some(""), Some("   ")] {
        assert_eq!(select_steam_root(&trusted, caller), Some(first.clone()));
    }
}

#[test]
fn no_trusted_roots_selects_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let other = dir(tmp.path(), "Other");

    assert_eq!(select_steam_root(&[], Some(&text(&other))), None);
    assert_eq!(select_steam_root(&[], None), None);
}
