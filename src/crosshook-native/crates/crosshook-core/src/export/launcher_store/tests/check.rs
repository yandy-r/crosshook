//! Tests for `check_launcher_exists`, `check_launcher_exists_for_request`,
//! `check_launcher_for_profile`, and `parse_display_name_from_desktop_content`.

use std::fs;
use std::path::Path;

use tempfile::tempdir;

use crate::export::launcher::{
    build_desktop_entry_content, build_trainer_script_content, SteamExternalLauncherExportRequest,
};
use crate::profile::{GameProfile, TrainerLoadingMode};
use crate::profile::{
    GameSection, LaunchSection, LauncherSection, RuntimeSection, SteamSection, TrainerSection,
};
use crate::settings::UmuPreference;

use super::super::fs_ops::parse_display_name_from_desktop_content;
use super::super::queries::{
    check_launcher_exists, check_launcher_exists_for_request, check_launcher_for_profile,
};
use super::{create_file_at, create_file_with_content, derive_test_paths};

// --- check_launcher_exists tests ---

#[test]
fn check_when_both_files_exist() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, desktop_path) = derive_test_paths(&home, "Test Game");

    create_file_at(&script_path);
    create_file_at(&desktop_path);

    let info = check_launcher_exists("Test Game", "", "/fake/trainer.exe", &home, "")
        .expect("check launcher exists");
    assert!(info.script_exists, "script should exist");
    assert!(info.desktop_entry_exists, "desktop entry should exist");
    assert!(!info.script_path.is_empty());
    assert!(!info.desktop_entry_path.is_empty());
    assert!(
        info.is_stale,
        "placeholder desktop content should be treated as stale"
    );
}

#[test]
fn check_when_neither_exists() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    let info = check_launcher_exists("Test Game", "", "/fake/trainer.exe", &home, "")
        .expect("check launcher exists");
    assert!(!info.script_exists, "script should not exist");
    assert!(!info.desktop_entry_exists, "desktop entry should not exist");
}

#[test]
fn check_launcher_exists_for_request_marks_mode_mismatches_as_stale() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let display_name = "Aurora Test";
    let request = SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        launcher_name: display_name.to_string(),
        trainer_path: "/opt/trainers/Aurora.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        launcher_icon_path: String::new(),
        prefix_path: "/games/prefixes/aurora-test".to_string(),
        proton_path: "/opt/proton/proton".to_string(),
        steam_app_id: String::new(),
        steam_client_install_path: String::new(),
        target_home_path: home.clone(),
        profile_name: None,
        ..Default::default()
    };

    let info =
        check_launcher_exists_for_request(display_name, &request).expect("derive launcher paths");
    let copy_request = SteamExternalLauncherExportRequest {
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        ..request.clone()
    };

    create_file_with_content(
        &info.script_path,
        &build_trainer_script_content(&copy_request, display_name),
    );
    create_file_with_content(
        &info.desktop_entry_path,
        &build_desktop_entry_content(display_name, &info.launcher_slug, &info.script_path, ""),
    );

    let info =
        check_launcher_exists_for_request(display_name, &request).expect("check launcher request");
    assert!(info.script_exists);
    assert!(info.desktop_entry_exists);
    assert!(info.is_stale, "mode mismatch should mark launcher stale");
}

#[test]
fn check_when_only_script_exists() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, _desktop_path) = derive_test_paths(&home, "Partial Game");

    create_file_at(&script_path);

    let info = check_launcher_exists("Partial Game", "", "/fake/trainer.exe", &home, "")
        .expect("check launcher exists");
    assert!(info.script_exists, "script should exist");
    assert!(!info.desktop_entry_exists, "desktop entry should not exist");
}

#[test]
fn check_marks_launcher_stale_when_display_name_mismatches() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, desktop_path) = derive_test_paths(&home, "Expected Game");

    create_file_at(&script_path);
    create_file_with_content(
        &desktop_path,
        "[Desktop Entry]\nName=Different Game - Trainer\nExec=/bin/bash trainer.sh\n",
    );

    let info = check_launcher_exists("Expected Game", "", "/fake/trainer.exe", &home, "")
        .expect("check launcher exists");

    assert!(info.is_stale);
}

#[test]
fn check_returns_error_when_desktop_entry_cannot_be_read() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, desktop_path) = derive_test_paths(&home, "Encoded Game");

    create_file_at(&script_path);
    let parent = Path::new(&desktop_path).parent().expect("desktop parent");
    fs::create_dir_all(parent).expect("desktop parent dirs");
    fs::write(&desktop_path, [0xff, 0xfe, 0xfd]).expect("write invalid desktop bytes");

    let error = check_launcher_exists("Encoded Game", "", "/fake/trainer.exe", &home, "")
        .expect_err("invalid desktop bytes should now surface as an error");

    assert!(matches!(
        error,
        super::super::types::LauncherStoreError::Io(_)
    ));
}

#[test]
fn check_launcher_for_profile_delegates_correctly() {
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

    let direct = check_launcher_exists(
        &profile.steam.launcher.display_name,
        &profile.steam.app_id,
        &profile.trainer.path,
        &home,
        "",
    )
    .expect("check launcher exists");
    let delegated = check_launcher_for_profile(&profile, &home, "", UmuPreference::Auto)
        .expect("check launcher for profile");

    assert_eq!(delegated, direct);
}

#[test]
fn check_launcher_for_profile_resolves_legacy_empty_method_as_steam() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    let profile = GameProfile {
        game: GameSection {
            name: "Legacy Steam Game".to_string(),
            executable_path: "/games/legacy/launcher.exe".to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection {
            path: "/mnt/trainers/legacy.exe".to_string(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        steam: SteamSection {
            enabled: true,
            app_id: "12345".to_string(),
            compatdata_path: "/steam/compatdata/12345".to_string(),
            proton_path: "/steam/proton/proton".to_string(),
            launcher: LauncherSection {
                display_name: "Legacy Steam Game".to_string(),
                icon_path: String::new(),
            },
        },
        runtime: RuntimeSection {
            prefix_path: "/wrong/runtime/prefix".to_string(),
            proton_path: "/wrong/runtime/proton".to_string(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
            umu_preference: None,
        },
        launch: LaunchSection {
            method: String::new(),
            ..Default::default()
        },
        ..Default::default()
    };

    let direct = check_launcher_exists_for_request(
        &profile.steam.launcher.display_name,
        &SteamExternalLauncherExportRequest {
            method: "steam_applaunch".to_string(),
            launcher_name: profile.steam.launcher.display_name.clone(),
            trainer_path: profile.trainer.path.clone(),
            trainer_loading_mode: profile.trainer.loading_mode,
            launcher_icon_path: profile.steam.launcher.icon_path.clone(),
            prefix_path: profile.steam.compatdata_path.clone(),
            proton_path: profile.steam.proton_path.clone(),
            steam_app_id: profile.steam.app_id.clone(),
            steam_client_install_path: String::new(),
            target_home_path: home.clone(),
            profile_name: None,
            ..Default::default()
        },
    )
    .expect("check launcher exists for explicit steam request");

    let delegated = check_launcher_for_profile(&profile, &home, "", UmuPreference::Auto)
        .expect("check launcher for profile");

    assert_eq!(delegated, direct);
}

// --- parse_display_name_from_desktop_content tests ---

#[test]
fn parse_display_name_handles_name_without_suffix() {
    assert_eq!(
        parse_display_name_from_desktop_content("[Desktop Entry]\nName=Standalone Launcher\n"),
        Some("Standalone Launcher".to_string())
    );
}

#[test]
fn parse_display_name_returns_none_without_name_line() {
    assert_eq!(
        parse_display_name_from_desktop_content("[Desktop Entry]\nExec=/bin/bash launcher.sh\n"),
        None
    );
}

#[test]
fn check_treats_symlink_as_nonexistent() {
    use std::os::unix::fs as unix_fs;

    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, _desktop_path) = derive_test_paths(&home, "Symlink Test");

    // Create a symlink at the script path
    let real_file = temp.path().join("real-script.sh");
    fs::write(&real_file, "#!/usr/bin/env bash\n# real file").expect("write real file");
    let parent = Path::new(&script_path).parent().expect("parent");
    fs::create_dir_all(parent).expect("mkdir");
    unix_fs::symlink(&real_file, &script_path).expect("symlink");

    let info = check_launcher_exists("Symlink Test", "", "/fake/trainer.exe", &home, "")
        .expect("check launcher exists");

    // Symlinks should not be reported as existing
    assert!(
        !info.script_exists,
        "symlink should not be reported as existing"
    );
}

#[test]
fn check_treats_directory_as_nonexistent() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();
    let (script_path, _desktop_path) = derive_test_paths(&home, "Directory Test");

    // Create a directory at the script path
    fs::create_dir_all(&script_path).expect("create directory");

    let info = check_launcher_exists("Directory Test", "", "/fake/trainer.exe", &home, "")
        .expect("check launcher exists");

    // Directories should not be reported as existing
    assert!(
        !info.script_exists,
        "directory should not be reported as existing"
    );
}
