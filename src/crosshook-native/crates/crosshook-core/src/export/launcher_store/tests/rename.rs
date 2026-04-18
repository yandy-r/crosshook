//! Tests for `rename_launcher_files`.

use std::fs;
use std::path::Path;

use tempfile::tempdir;

use crate::export::launcher::combine_host_unix_path;

use super::super::mutations::rename_launcher_files;
use super::{
    create_file_with_content, create_watermarked_desktop, create_watermarked_script,
    make_test_request,
};

#[test]
fn rename_when_old_exists() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let request = make_test_request();

    let old_slug = "old-game";
    let old_script = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{old_slug}-trainer.sh"),
    );
    let old_desktop = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{old_slug}-trainer.desktop"),
    );

    create_watermarked_script(&old_script);
    create_watermarked_desktop(&old_desktop);

    let result = rename_launcher_files(old_slug, "New Game", "", &home, "", &request)
        .expect("rename should succeed");

    assert_eq!(result.old_slug, "old-game");
    assert_eq!(result.new_slug, "new-game");
    assert!(result.script_renamed, "script should be renamed");
    assert!(
        result.desktop_entry_renamed,
        "desktop entry should be renamed"
    );

    // Old files should be deleted (slug changed)
    assert!(
        !Path::new(&old_script).exists(),
        "old script should be removed"
    );
    assert!(
        !Path::new(&old_desktop).exists(),
        "old desktop entry should be removed"
    );

    // New files should exist with updated content
    assert!(
        Path::new(&result.new_script_path).exists(),
        "new script should exist"
    );
    assert!(
        Path::new(&result.new_desktop_entry_path).exists(),
        "new desktop entry should exist"
    );

    let new_script_content = fs::read_to_string(&result.new_script_path).expect("read new script");
    assert!(
        new_script_content.contains("# New Game - Trainer launcher"),
        "new script should contain updated display name"
    );

    let new_desktop_content =
        fs::read_to_string(&result.new_desktop_entry_path).expect("read new desktop");
    assert!(
        new_desktop_content.contains("Name=New Game - Trainer"),
        "new desktop should contain updated display name"
    );
}

#[test]
fn rename_when_old_does_not_exist() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let request = make_test_request();

    let result = rename_launcher_files("nonexistent-game", "New Name", "", &home, "", &request)
        .expect("rename should succeed with no-op");

    assert!(!result.script_renamed, "script should not be renamed");
    assert!(
        !result.desktop_entry_renamed,
        "desktop entry should not be renamed"
    );
    assert_eq!(result.old_slug, "nonexistent-game");
    assert_eq!(result.new_slug, "new-name");
}

#[test]
fn rename_when_slug_unchanged() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let request = make_test_request();

    // The slug for "My Game" is "my-game". If we rename from "my-game" to "My Game",
    // the slug stays the same -- content should be rewritten in place without deleting.
    let slug = "my-game";
    let script_path = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{slug}-trainer.sh"),
    );
    let desktop_path = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{slug}-trainer.desktop"),
    );

    create_file_with_content(&script_path, "#!/usr/bin/env bash\n# old content");
    create_file_with_content(
        &desktop_path,
        "[Desktop Entry]\nName=Old Name - Trainer\nExec=/bin/bash old.sh\n",
    );

    let result = rename_launcher_files(slug, "My Game", "", &home, "", &request)
        .expect("rename should succeed");

    assert_eq!(result.old_slug, "my-game");
    assert_eq!(result.new_slug, "my-game");
    assert!(result.script_renamed, "script should be rewritten");
    assert!(
        result.desktop_entry_renamed,
        "desktop entry should be rewritten"
    );

    // Files should still exist at the same paths (not deleted)
    assert!(
        Path::new(&script_path).exists(),
        "script should still exist at same path"
    );
    assert!(
        Path::new(&desktop_path).exists(),
        "desktop entry should still exist at same path"
    );

    // Content should be updated
    let new_script_content = fs::read_to_string(&script_path).expect("read script");
    assert!(
        new_script_content.contains("# My Game - Trainer launcher"),
        "script content should be updated with new display name"
    );

    let new_desktop_content = fs::read_to_string(&desktop_path).expect("read desktop");
    assert!(
        new_desktop_content.contains("Name=My Game - Trainer"),
        "desktop content should be updated with new display name"
    );
}

#[test]
fn rename_when_only_script_exists() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let request = make_test_request();
    let old_slug = "script-only";
    let old_script = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{old_slug}-trainer.sh"),
    );

    create_watermarked_script(&old_script);

    let result = rename_launcher_files(old_slug, "Script Only Renamed", "", &home, "", &request)
        .expect("rename should succeed");

    assert!(result.script_renamed);
    assert!(!result.desktop_entry_renamed);
    assert!(Path::new(&result.new_script_path).exists());
    assert!(!Path::new(&old_script).exists());
}

#[test]
fn rename_when_only_desktop_entry_exists() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let request = make_test_request();
    let old_slug = "desktop-only";
    let old_desktop = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{old_slug}-trainer.desktop"),
    );

    create_watermarked_desktop(&old_desktop);

    let result = rename_launcher_files(old_slug, "Desktop Only Renamed", "", &home, "", &request)
        .expect("rename should succeed");

    assert!(!result.script_renamed);
    assert!(result.desktop_entry_renamed);
    assert!(Path::new(&result.new_desktop_entry_path).exists());
    assert!(!Path::new(&old_desktop).exists());
}

#[test]
fn rename_reports_warning_when_old_file_is_not_crosshook_managed() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let request = make_test_request();
    let old_slug = "unsafe-old";
    let old_script = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{old_slug}-trainer.sh"),
    );
    let old_desktop = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{old_slug}-trainer.desktop"),
    );

    create_file_with_content(&old_script, "#!/usr/bin/env bash\necho unmanaged\n");
    create_watermarked_desktop(&old_desktop);

    let result = rename_launcher_files(old_slug, "Safe New Name", "", &home, "", &request)
        .expect("rename should succeed");

    assert!(result.script_renamed);
    assert!(result.desktop_entry_renamed);
    assert!(result.old_script_cleanup_warning.is_some());
    assert!(result.old_desktop_entry_cleanup_warning.is_none());
    assert!(
        Path::new(&old_script).exists(),
        "unsafe old script should remain"
    );
}
