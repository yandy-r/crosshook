//! Directory detection: the no-game-path gate, DB-down scanning and
//! best-effort `already_registered` enrichment.

use crosshook_core::mods::{ModCategory, ProfileModInput};
use crosshook_core::profile::GameProfile;

use super::helpers::{mod_input, profile_with_exe, Harness, Metadata};
use crate::AppErrorKind;

const NO_GAME_PATH: &str = "no_game_path: set the game executable path to enable detection";

fn no_game_path_kind() -> AppErrorKind {
    AppErrorKind::Validation { field: "game_path" }
}

fn candidate<'a>(
    report: &'a crosshook_core::mods::DetectionScanReport,
    detector_id: &str,
) -> &'a crosshook_core::mods::DetectedModCandidate {
    report
        .candidates
        .iter()
        .find(|c| c.detector_id == detector_id)
        .unwrap_or_else(|| panic!("no candidate {detector_id}"))
}

#[tokio::test]
async fn empty_executable_path_is_no_game_path() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("alpha", GameProfile::default(), true);

    let error = harness.service.detect("alpha".into()).await.unwrap_err();

    assert_eq!(error.kind(), no_game_path_kind());
    assert_eq!(error.message(), NO_GAME_PATH);
}

#[tokio::test]
async fn whitespace_executable_path_is_no_game_path() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("alpha", profile_with_exe("   "), true);

    let error = harness.service.detect("alpha".into()).await.unwrap_err();

    assert_eq!(error.kind(), no_game_path_kind());
    assert_eq!(error.message(), NO_GAME_PATH);
}

#[tokio::test]
async fn executable_without_a_parent_directory_is_no_game_path() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("alpha", profile_with_exe("game.exe"), true);

    let error = harness.service.detect("alpha".into()).await.unwrap_err();

    assert_eq!(error.kind(), no_game_path_kind());
    assert_eq!(error.message(), NO_GAME_PATH);
}

#[tokio::test]
async fn missing_profile_is_not_found() {
    let harness = Harness::new(Metadata::Enabled);

    let error = harness.service.detect("ghost".into()).await.unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::NotFound);
}

#[tokio::test]
async fn unreadable_game_directory_is_an_io_error() {
    let harness = Harness::new(Metadata::Enabled);
    // `<root>/game` is never created.
    harness.seed_profile("alpha", &harness.game_exe());

    let error = harness.service.detect("alpha".into()).await.unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Io);
}

#[tokio::test]
async fn detect_scans_with_the_database_down_and_leaves_flags_false() {
    let harness = Harness::new(Metadata::Disabled);
    harness.touch_game_file("ReShade.ini");
    harness.touch_game_file("skse64_loader.exe");
    harness.seed_profile("alpha", &harness.game_exe());

    let report = harness.service.detect("alpha".into()).await.unwrap();

    assert_eq!(
        report.scanned_root,
        harness.game_dir().to_string_lossy().as_ref()
    );
    assert_eq!(report.candidates.len(), 2);
    assert!(report.candidates.iter().all(|c| !c.already_registered));
    assert_eq!(
        candidate(&report, "script_extender:skse64").category,
        ModCategory::ScriptExtender
    );
}

#[tokio::test]
async fn flags_are_true_only_for_registered_names() {
    let harness = Harness::new(Metadata::Enabled);
    harness.touch_game_file("ReShade.ini");
    harness.touch_game_file("enblocal.ini");
    harness.seed_profile("alpha", &harness.game_exe());
    // Case-insensitive name match on the suggested name "ReShade".
    harness
        .service
        .add("alpha".into(), mod_input("reshade"))
        .await
        .unwrap();

    let report = harness.service.detect("alpha".into()).await.unwrap();

    assert!(candidate(&report, "reshade").already_registered);
    assert!(!candidate(&report, "enb").already_registered);
}

#[tokio::test]
async fn flags_follow_same_category_shared_paths() {
    let harness = Harness::new(Metadata::Enabled);
    harness.touch_game_file("enblocal.ini");
    harness.seed_profile("alpha", &harness.game_exe());
    harness
        .service
        .add(
            "alpha".into(),
            ProfileModInput {
                category: ModCategory::OverlayInjection,
                paths: vec![harness
                    .game_dir()
                    .join("ENBLocal.ini")
                    .to_string_lossy()
                    .into_owned()],
                ..mod_input("My graphics preset")
            },
        )
        .await
        .unwrap();

    let report = harness.service.detect("alpha".into()).await.unwrap();

    assert!(candidate(&report, "enb").already_registered);
}

#[tokio::test]
async fn mods_registered_on_another_profile_do_not_flag_candidates() {
    let harness = Harness::new(Metadata::Enabled);
    harness.touch_game_file("ReShade.ini");
    harness.seed_profile("alpha", &harness.game_exe());
    harness.seed_profile("beta", &harness.game_exe());
    harness
        .service
        .add("beta".into(), mod_input("ReShade"))
        .await
        .unwrap();

    let report = harness.service.detect("alpha".into()).await.unwrap();

    assert!(!candidate(&report, "reshade").already_registered);
}

#[tokio::test]
async fn unregistered_profile_scans_with_flags_false() {
    let harness = Harness::new(Metadata::Enabled);
    harness.touch_game_file("ReShade.ini");
    harness.seed_profile_unregistered("alpha", &harness.game_exe());

    let report = harness.service.detect("alpha".into()).await.unwrap();

    assert!(!candidate(&report, "reshade").already_registered);
}
