#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::cleanup_prefix_storage;
use crate::storage::prefix_health::{
    PrefixCleanupResult, PrefixCleanupTarget, PrefixCleanupTargetKind, PrefixReference,
};

fn reference(name: &str, resolved: &Path) -> PrefixReference {
    PrefixReference {
        profile_name: name.to_string(),
        configured_prefix_path: resolved.to_string_lossy().into_owned(),
        resolved_prefix_path: resolved.to_string_lossy().into_owned(),
    }
}

fn orphan_target(prefix: &Path) -> PrefixCleanupTarget {
    PrefixCleanupTarget {
        kind: PrefixCleanupTargetKind::OrphanPrefix,
        resolved_prefix_path: prefix.to_string_lossy().into_owned(),
        target_path: prefix.to_string_lossy().into_owned(),
    }
}

fn staged_target(prefix: &Path, target: &Path) -> PrefixCleanupTarget {
    PrefixCleanupTarget {
        kind: PrefixCleanupTargetKind::StaleStagedTrainer,
        resolved_prefix_path: prefix.to_string_lossy().into_owned(),
        target_path: target.to_string_lossy().into_owned(),
    }
}

fn make_prefix(path: &Path) {
    fs::create_dir_all(path.join("drive_c/CrossHook/StagedTrainers")).unwrap();
    fs::write(path.join("drive_c/CrossHook/marker.txt"), b"managed").unwrap();
}

fn staged_root(prefix: &Path) -> PathBuf {
    prefix.join("drive_c/CrossHook/StagedTrainers")
}

fn assert_skipped(result: &PrefixCleanupResult, reason: &str) {
    assert!(
        result.deleted.is_empty(),
        "expected no deletions, got {:?}",
        result.deleted
    );
    assert_eq!(result.skipped.len(), 1, "skipped: {:?}", result.skipped);
    assert_eq!(result.skipped[0].reason, reason);
}

/// Root layout: `<tmp>/root/{ref,orphan,...}`; trusted root is `<tmp>/root`.
struct Layout {
    _tmp: TempDir,
    root: PathBuf,
}

fn layout() -> Layout {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("root");
    fs::create_dir_all(&root).unwrap();
    Layout { _tmp: tmp, root }
}

#[test]
fn orphan_happy_path_deletes_inside_trusted_root() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    let orphan = layout.root.join("orphan");
    make_prefix(&referenced);
    make_prefix(&orphan);
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&orphan)]);
    assert_eq!(result.deleted.len(), 1);
    assert!(result.skipped.is_empty());
    assert!(!orphan.exists());
    assert!(referenced.exists());
}

#[test]
fn orphan_compatdata_trusted_root_is_grandparent_of_pfx() {
    let tmp = TempDir::new().unwrap();
    let compatdata = tmp.path().join("steam/compatdata");
    let referenced = compatdata.join("111/pfx");
    let orphan = compatdata.join("222/pfx");
    make_prefix(&referenced);
    make_prefix(&orphan);
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&orphan)]);
    assert_eq!(result.deleted.len(), 1);
    assert!(!orphan.exists());
}

#[test]
fn orphan_symlink_target_in_root_referent_survives() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    let real = layout.root.join("real");
    make_prefix(&referenced);
    make_prefix(&real);
    let link = layout.root.join("link");
    symlink(&real, &link).unwrap();
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&link)]);
    assert_skipped(&result, "target is a symlink; refusing cleanup");
    assert!(real.join("drive_c/CrossHook/marker.txt").exists());
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn orphan_symlink_target_outside_root_referent_survives() {
    let layout = layout();
    let outside = TempDir::new().unwrap();
    let external = outside.path().join("external");
    make_prefix(&external);
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let link = layout.root.join("link");
    symlink(&external, &link).unwrap();
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&link)]);
    assert_skipped(&result, "target is a symlink; refusing cleanup");
    assert!(external.join("drive_c/CrossHook/marker.txt").exists());
}

#[test]
fn orphan_dangling_symlink_is_skipped() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let link = layout.root.join("dangling");
    symlink(layout.root.join("missing"), &link).unwrap();
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&link)]);
    assert_skipped(&result, "target is a symlink; refusing cleanup");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn orphan_missing_target_reports_no_longer_exists() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let missing = layout.root.join("missing");
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&missing)]);
    assert_skipped(&result, "target no longer exists");
}

#[test]
fn orphan_non_directory_target_is_skipped() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let file = layout.root.join("file");
    fs::write(&file, b"x").unwrap();
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&file)]);
    assert_skipped(&result, "target is not a directory; refusing cleanup");
    assert!(file.exists());
}

#[test]
fn orphan_symlinked_drive_c_intermediate_is_skipped() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let external = layout.root.join("external-drive");
    fs::create_dir_all(external.join("CrossHook")).unwrap();
    fs::write(external.join("CrossHook/marker.txt"), b"managed").unwrap();
    let orphan = layout.root.join("orphan");
    fs::create_dir_all(&orphan).unwrap();
    symlink(&external, orphan.join("drive_c")).unwrap();
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&orphan)]);
    assert_skipped(
        &result,
        "intermediate path component is a symlink; refusing cleanup",
    );
    assert!(external.join("CrossHook/marker.txt").exists());
    assert!(orphan.exists());
}

#[test]
fn orphan_symlinked_crosshook_intermediate_is_skipped() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let external = layout.root.join("external-crosshook");
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("marker.txt"), b"managed").unwrap();
    let orphan = layout.root.join("orphan");
    fs::create_dir_all(orphan.join("drive_c")).unwrap();
    symlink(&external, orphan.join("drive_c/CrossHook")).unwrap();
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&orphan)]);
    assert_skipped(
        &result,
        "intermediate path component is a symlink; refusing cleanup",
    );
    assert!(external.join("marker.txt").exists());
    assert!(orphan.exists());
}

#[test]
fn orphan_target_equal_to_trusted_root_is_skipped() {
    let layout = layout();
    // The trusted root itself looks like a managed prefix.
    make_prefix(&layout.root);
    let referenced = layout.root.join("ref");
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&layout.root)]);
    assert_skipped(
        &result,
        "target is not strictly inside a trusted prefix root",
    );
    assert!(layout.root.join("drive_c/CrossHook/marker.txt").exists());
}

#[test]
fn orphan_outside_trusted_root_is_skipped() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let elsewhere = layout.root.parent().unwrap().join("elsewhere");
    // `elsewhere` is a sibling of the trusted root, under the tempdir.
    let orphan = elsewhere.join("orphan");
    make_prefix(&orphan);
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&orphan)]);
    assert_skipped(
        &result,
        "target is not strictly inside a trusted prefix root",
    );
    assert!(orphan.exists());
}

#[test]
fn orphan_trusted_root_canonicalize_failure_fails_closed() {
    let layout = layout();
    let orphan = layout.root.join("orphan");
    make_prefix(&orphan);
    // The only reference resolves under a root that cannot be canonicalized.
    let bogus = PathBuf::from("/crosshook-nonexistent-root-xyz/ref");
    let refs = vec![reference("a", &bogus)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&orphan)]);
    assert_skipped(&result, "failed to canonicalize trusted prefix root");
    assert!(orphan.exists());
}

#[test]
fn orphan_still_referenced_is_skipped() {
    let layout = layout();
    let referenced = layout.root.join("ref");
    make_prefix(&referenced);
    let refs = vec![reference("a", &referenced)];
    let result = cleanup_prefix_storage(&refs, &[orphan_target(&referenced)]);
    assert_skipped(
        &result,
        "prefix is still referenced by at least one profile",
    );
    assert!(referenced.exists());
}

#[test]
fn staged_happy_path_deletes_immediate_child_file() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let entry = staged_root(&prefix).join("trainer.exe");
    fs::write(&entry, b"trainer").unwrap();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &entry)]);
    assert_eq!(result.deleted.len(), 1);
    assert!(!entry.exists());
    assert!(prefix.exists());
}

#[test]
fn staged_symlink_target_in_root_referent_survives() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let real = staged_root(&prefix).join("real.exe");
    fs::write(&real, b"real").unwrap();
    let link = staged_root(&prefix).join("link.exe");
    symlink(&real, &link).unwrap();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &link)]);
    assert_skipped(&result, "target is a symlink; refusing cleanup");
    assert!(real.exists());
}

#[test]
fn staged_symlink_target_outside_root_referent_survives() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let outside = TempDir::new().unwrap();
    let external = outside.path().join("external.exe");
    fs::write(&external, b"external").unwrap();
    let link = staged_root(&prefix).join("link.exe");
    symlink(&external, &link).unwrap();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &link)]);
    assert_skipped(&result, "target is a symlink; refusing cleanup");
    assert!(external.exists());
}

#[test]
fn staged_dangling_symlink_is_skipped() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let link = staged_root(&prefix).join("dangling.exe");
    symlink(staged_root(&prefix).join("missing.exe"), &link).unwrap();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &link)]);
    assert_skipped(&result, "target is a symlink; refusing cleanup");
}

#[test]
fn staged_missing_target_reports_no_longer_exists() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let missing = staged_root(&prefix).join("missing.exe");
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &missing)]);
    assert_skipped(&result, "staged trainer entry no longer exists");
}

#[test]
fn staged_symlinked_drive_c_intermediate_is_skipped() {
    let layout = layout();
    let external_drive = layout.root.join("external-drive");
    fs::create_dir_all(external_drive.join("CrossHook/StagedTrainers")).unwrap();
    let external_entry = external_drive.join("CrossHook/StagedTrainers/trainer.exe");
    fs::write(&external_entry, b"external").unwrap();
    let prefix = layout.root.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    symlink(&external_drive, prefix.join("drive_c")).unwrap();
    let entry = staged_root(&prefix).join("trainer.exe");
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &entry)]);
    assert_skipped(
        &result,
        "intermediate path component is a symlink; refusing cleanup",
    );
    assert!(external_entry.exists());
}

#[test]
fn staged_symlinked_crosshook_intermediate_is_skipped() {
    let layout = layout();
    let external = layout.root.join("external-crosshook");
    fs::create_dir_all(external.join("StagedTrainers")).unwrap();
    let external_entry = external.join("StagedTrainers/trainer.exe");
    fs::write(&external_entry, b"external").unwrap();
    let prefix = layout.root.join("prefix");
    fs::create_dir_all(prefix.join("drive_c")).unwrap();
    symlink(&external, prefix.join("drive_c/CrossHook")).unwrap();
    let entry = staged_root(&prefix).join("trainer.exe");
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &entry)]);
    assert_skipped(
        &result,
        "intermediate path component is a symlink; refusing cleanup",
    );
    assert!(external_entry.exists());
}

#[test]
fn staged_symlinked_staged_trainers_root_is_skipped() {
    let layout = layout();
    let external = layout.root.join("external-staged");
    fs::create_dir_all(&external).unwrap();
    let external_entry = external.join("trainer.exe");
    fs::write(&external_entry, b"external").unwrap();
    let prefix = layout.root.join("prefix");
    fs::create_dir_all(prefix.join("drive_c/CrossHook")).unwrap();
    symlink(&external, staged_root(&prefix)).unwrap();
    let entry = staged_root(&prefix).join("trainer.exe");
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &entry)]);
    assert_skipped(
        &result,
        "staged trainers root is a symlink; refusing cleanup",
    );
    assert!(external_entry.exists());
}

#[test]
fn staged_target_equal_to_staged_root_is_skipped() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let root = staged_root(&prefix);
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &root)]);
    assert_skipped(&result, "target is a cleanup root; refusing cleanup");
    assert!(root.exists());
}

#[test]
fn staged_target_equal_to_prefix_is_skipped() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let target = prefix.clone();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &target)]);
    assert_skipped(&result, "target is a cleanup root; refusing cleanup");
    assert!(prefix.exists());
}

#[test]
fn staged_non_immediate_child_is_skipped() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let nested = staged_root(&prefix).join("sub/trainer.exe");
    fs::create_dir_all(nested.parent().unwrap()).unwrap();
    fs::write(&nested, b"nested").unwrap();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &nested)]);
    assert_skipped(
        &result,
        "target is not an immediate child of the staged trainers root",
    );
    assert!(nested.exists());
}

#[test]
fn staged_target_outside_staged_root_is_skipped() {
    let layout = layout();
    let prefix = layout.root.join("prefix");
    make_prefix(&prefix);
    let outside_entry = prefix.join("drive_c/CrossHook/other.exe");
    fs::write(&outside_entry, b"outside").unwrap();
    let result = cleanup_prefix_storage(&[], &[staged_target(&prefix, &outside_entry)]);
    // Lexically outside the verified staged chain / not an immediate child.
    assert_eq!(result.deleted.len(), 0);
    assert_eq!(result.skipped.len(), 1);
    assert!(outside_entry.exists());
}
