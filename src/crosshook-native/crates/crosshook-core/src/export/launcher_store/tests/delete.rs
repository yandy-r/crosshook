//! Tests for `delete_launcher_files`, `delete_launcher_by_slug`,
//! `delete_launcher_for_profile`, and watermark/symlink verification behaviour.

use std::fs;
use std::path::Path;

use tempfile::tempdir;

use crate::profile::{
    GameProfile, GameSection, LaunchSection, LauncherSection, SteamSection, TrainerLoadingMode,
    TrainerSection,
};

use super::super::mutations::{
    delete_launcher_by_slug, delete_launcher_files, delete_launcher_for_profile,
};
use super::super::paths::derive_launcher_paths_from_slug;
use super::super::queries::check_launcher_exists;
use super::{
    create_file_at, create_watermarked_desktop, create_watermarked_script, derive_test_paths,
};

// --- delete_launcher_files tests ---

#[test]
fn delete_when_both_exist() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, desktop_path) = derive_test_paths(&home, "Delete Both");

    create_watermarked_script(&script_path);
    create_watermarked_desktop(&desktop_path);

    let result = delete_launcher_files("Delete Both", "", "/fake/trainer.exe", &home, "")
        .expect("delete should succeed");

    assert!(result.script_deleted, "script should be deleted");
    assert!(
        result.desktop_entry_deleted,
        "desktop entry should be deleted"
    );
    assert!(
        !Path::new(&result.script_path).exists(),
        "script file should be gone"
    );
    assert!(
        !Path::new(&result.desktop_entry_path).exists(),
        "desktop entry file should be gone"
    );
}

#[test]
fn delete_when_neither_exists_is_noop() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    let result = delete_launcher_files("No Files", "", "/fake/trainer.exe", &home, "")
        .expect("delete should succeed even with no files");

    assert!(!result.script_deleted, "nothing to delete for script");
    assert!(
        !result.desktop_entry_deleted,
        "nothing to delete for desktop entry"
    );
}

#[test]
fn delete_when_only_script_exists() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, _desktop_path) = derive_test_paths(&home, "Script Only");

    create_watermarked_script(&script_path);

    let result = delete_launcher_files("Script Only", "", "/fake/trainer.exe", &home, "")
        .expect("delete should succeed");

    assert!(result.script_deleted, "script should be deleted");
    assert!(
        !result.desktop_entry_deleted,
        "desktop entry was not present"
    );
    assert!(
        !Path::new(&result.script_path).exists(),
        "script file should be gone"
    );
}

#[test]
fn delete_launcher_for_profile_delegates_correctly() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    let profile = GameProfile {
        game: GameSection {
            name: "Test Game".to_string(),
            executable_path: String::new(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection {
            path: "/mnt/trainers/test.exe".to_string(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        steam: SteamSection {
            app_id: "12345".to_string(),
            launcher: LauncherSection {
                display_name: "Test Game".to_string(),
                icon_path: String::new(),
            },
            ..Default::default()
        },
        launch: LaunchSection {
            method: "steam_applaunch".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    // Derive paths using the same inputs the profile facade will use
    let info = check_launcher_exists(
        &profile.steam.launcher.display_name,
        &profile.steam.app_id,
        &profile.trainer.path,
        &home,
        "",
    )
    .expect("derive launcher paths");

    create_watermarked_script(&info.script_path);
    create_watermarked_desktop(&info.desktop_entry_path);

    let result = delete_launcher_for_profile(&profile, &home, "")
        .expect("delete for profile should succeed");

    assert!(result.script_deleted);
    assert!(result.desktop_entry_deleted);
    assert!(!Path::new(&result.script_path).exists());
    assert!(!Path::new(&result.desktop_entry_path).exists());
}

#[test]
fn delete_launcher_by_slug_deletes_matching_files() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    let (script_path, desktop_path) = derive_launcher_paths_from_slug("delete-by-slug", &home, "");

    create_watermarked_script(&script_path);
    create_watermarked_desktop(&desktop_path);

    let result = delete_launcher_by_slug("delete-by-slug", &home, "")
        .expect("delete by slug should succeed");

    assert!(result.script_deleted);
    assert!(result.desktop_entry_deleted);
    assert_eq!(result.script_path, script_path);
    assert_eq!(result.desktop_entry_path, desktop_path);
    assert!(!Path::new(&result.script_path).exists());
    assert!(!Path::new(&result.desktop_entry_path).exists());
}

// --- watermark verification tests ---

#[test]
fn delete_skips_symlink() {
    use std::os::unix::fs as unix_fs;
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, _desktop_path) = derive_test_paths(&home, "Symlink Test");

    let real_file = temp.path().join("real-script.sh");
    fs::write(
        &real_file,
        "#!/usr/bin/env bash\n# Generated by CrossHook\n",
    )
    .expect("write real");
    let parent = Path::new(&script_path).parent().expect("parent");
    fs::create_dir_all(parent).expect("mkdir");
    unix_fs::symlink(&real_file, &script_path).expect("symlink");

    let result = delete_launcher_files("Symlink Test", "", "/fake/trainer.exe", &home, "")
        .expect("delete should succeed");

    assert!(
        !result.script_deleted,
        "symlinked script should not be deleted"
    );
    assert!(
        result.script_skipped_reason.is_some(),
        "should have skip reason for symlink"
    );
    assert!(result
        .script_skipped_reason
        .as_ref()
        .unwrap()
        .contains("Not a regular file"),);
}

#[test]
fn delete_skips_file_without_watermark() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, desktop_path) = derive_test_paths(&home, "No Watermark");

    create_file_at(&script_path); // plain "placeholder", no watermark
    create_file_at(&desktop_path);

    let result = delete_launcher_files("No Watermark", "", "/fake/trainer.exe", &home, "")
        .expect("delete should succeed");

    assert!(!result.script_deleted);
    assert!(!result.desktop_entry_deleted);
    assert!(result.script_skipped_reason.is_some());
    assert!(result.desktop_entry_skipped_reason.is_some());
    assert!(Path::new(&script_path).exists(), "file should still exist");
    assert!(Path::new(&desktop_path).exists(), "file should still exist");
}

#[test]
fn delete_proceeds_with_watermarked_file() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, desktop_path) = derive_test_paths(&home, "Watermarked");

    create_watermarked_script(&script_path);
    create_watermarked_desktop(&desktop_path);

    let result = delete_launcher_files("Watermarked", "", "/fake/trainer.exe", &home, "")
        .expect("delete should succeed");

    assert!(result.script_deleted);
    assert!(result.desktop_entry_deleted);
    assert!(result.script_skipped_reason.is_none());
    assert!(result.desktop_entry_skipped_reason.is_none());
    assert!(!Path::new(&script_path).exists());
    assert!(!Path::new(&desktop_path).exists());
}
