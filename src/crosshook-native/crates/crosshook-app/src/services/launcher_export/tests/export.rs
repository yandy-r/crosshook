//! `LauncherExportService` tests: export slice.

use std::fs;
use std::os::unix::fs::symlink;

use super::helpers::{mode_of, Harness, Metadata};
use crate::AppErrorKind;

#[tokio::test]
async fn validate_accepts_a_valid_request_and_rejects_an_invalid_one() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .validate(harness.request("Valid"))
        .await
        .expect("valid request");

    let mut bad = harness.request("Bad");
    bad.method = "native".to_string();
    let error = harness.service.validate(bad).await.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "request" });
    assert!(error
        .message()
        .contains("only supports steam_applaunch and proton_run"));
}

#[tokio::test]
async fn export_writes_files_with_modes_and_observes_metadata() {
    let harness = Harness::new(Metadata::Enabled);
    let result = harness
        .service
        .export(harness.request("Mode Check"))
        .await
        .unwrap();

    assert_eq!(result.launcher_slug, "mode-check");
    assert_eq!(mode_of(&harness.script_path("mode-check")), 0o700);
    assert_eq!(mode_of(&harness.desktop_path("mode-check")), 0o644);
    assert_eq!(harness.launcher_rows("mode-check", "aligned"), 1);
}

#[tokio::test]
async fn export_succeeds_when_metadata_is_disabled() {
    let harness = Harness::new(Metadata::Disabled);
    let result = harness
        .service
        .export(harness.request("No Db"))
        .await
        .unwrap();
    assert!(harness.script_path(&result.launcher_slug).is_file());
    assert!(harness.desktop_path(&result.launcher_slug).is_file());
}

#[tokio::test]
async fn export_rejects_invalid_requests_without_writing() {
    let harness = Harness::new(Metadata::Enabled);
    let mut request = harness.request("Invalid");
    request.prefix_path = String::new();

    let error = harness.service.export(request).await.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "request" });
    assert_eq!(
        error.message(),
        "External launcher export requires a prefix path."
    );
    assert!(!harness.launchers_dir().exists());
}

#[tokio::test]
async fn check_exists_reports_files_and_staleness() {
    let harness = Harness::new(Metadata::Enabled);
    let request = harness.request("Check Me");

    let before = harness.service.check_exists(request.clone()).await.unwrap();
    assert!(!before.script_exists && !before.desktop_entry_exists && !before.is_stale);

    harness.service.export(request.clone()).await.unwrap();
    let after = harness.service.check_exists(request.clone()).await.unwrap();
    assert!(after.script_exists && after.desktop_entry_exists);
    assert!(!after.is_stale);
    assert_eq!(after.launcher_slug, "check-me");

    let mut changed = request;
    changed.network_isolation = true;
    let stale = harness.service.check_exists(changed).await.unwrap();
    assert!(stale.is_stale, "regenerated script differs");
}

#[tokio::test]
async fn check_for_profile_reports_missing_profile_and_native_default() {
    let harness = Harness::new(Metadata::Enabled);
    let missing = harness
        .service
        .check_for_profile("Nope".to_string())
        .await
        .unwrap_err();
    assert_eq!(missing.kind(), AppErrorKind::NotFound);

    let mut native = harness.save_profile("Native", "Native Launcher");
    native.launch.method = "native".to_string();
    harness.profiles.save("Native", &native).unwrap();
    let info = harness
        .service
        .check_for_profile("Native".to_string())
        .await
        .unwrap();
    assert_eq!(info, Default::default());
}

#[tokio::test]
async fn check_for_profile_hard_fails_on_settings_error() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("Alpha", "Alpha Launcher");
    harness.corrupt_settings();

    let error = harness
        .service
        .check_for_profile("Alpha".to_string())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "settings" });
}

#[tokio::test]
async fn delete_removes_watermarked_files_and_tombstones_metadata() {
    let harness = Harness::new(Metadata::Enabled);
    let request = harness.request("Delete Me");
    harness.service.export(request.clone()).await.unwrap();

    let result = harness
        .service
        .delete(
            request.launcher_name.clone(),
            request.steam_app_id.clone(),
            request.trainer_path.clone(),
            harness.home_str(),
            String::new(),
        )
        .await
        .unwrap();

    assert!(result.script_deleted && result.desktop_entry_deleted);
    assert!(!harness.script_path("delete-me").exists());
    assert_eq!(harness.launcher_rows("delete-me", "missing"), 1);
}

#[tokio::test]
async fn delete_by_slug_skips_unwatermarked_and_symlinked_files() {
    let harness = Harness::new(Metadata::Enabled);
    fs::create_dir_all(harness.launchers_dir()).unwrap();
    fs::create_dir_all(harness.applications_dir()).unwrap();
    fs::write(harness.script_path("foreign"), "#!/bin/sh\necho mine\n").unwrap();
    let target = harness.home.join("precious.desktop");
    fs::write(&target, "[Desktop Entry]\n").unwrap();
    symlink(&target, harness.desktop_path("foreign")).unwrap();

    let result = harness
        .service
        .delete_by_slug("foreign".to_string(), harness.home_str(), String::new())
        .await
        .unwrap();

    assert!(!result.script_deleted && !result.desktop_entry_deleted);
    assert!(result.script_skipped_reason.is_some());
    assert!(result.desktop_entry_skipped_reason.is_some());
    assert!(harness.script_path("foreign").exists());
    assert!(target.exists());
}

#[tokio::test]
async fn delete_by_slug_observes_with_the_raw_slug() {
    let harness = Harness::new(Metadata::Enabled);
    let result = harness
        .service
        .export(harness.request("Raw Slug"))
        .await
        .unwrap();
    assert_eq!(harness.launcher_rows(&result.launcher_slug, "aligned"), 1);

    harness
        .service
        .delete_by_slug(
            result.launcher_slug.clone(),
            harness.home_str(),
            String::new(),
        )
        .await
        .unwrap();
    assert_eq!(harness.launcher_rows(&result.launcher_slug, "missing"), 1);
}

#[tokio::test]
async fn rename_rewrites_files_and_removes_the_old_ones() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .export(harness.request("Old Name"))
        .await
        .unwrap();

    let result = harness
        .service
        .rename(harness.rename_request("old-name", "New Name"))
        .await
        .unwrap();

    assert!(result.script_renamed && result.desktop_entry_renamed);
    assert_eq!(result.new_slug, "new-name");
    assert!(harness.script_path("new-name").is_file());
    assert!(!harness.script_path("old-name").exists());
    assert!(!harness.desktop_path("old-name").exists());
    assert_eq!(harness.launcher_rows("old-name", "missing"), 1);
    assert_eq!(harness.launcher_rows("new-name", "aligned"), 1);
}

#[tokio::test]
async fn rename_without_old_files_reports_not_renamed() {
    let harness = Harness::new(Metadata::Disabled);
    let result = harness
        .service
        .rename(harness.rename_request("ghost", "Ghost Two"))
        .await
        .unwrap();
    assert!(!result.script_renamed && !result.desktop_entry_renamed);
    assert!(!harness.script_path("ghost-two").exists());
}

#[tokio::test]
async fn rename_rejects_an_unknown_loading_mode() {
    let harness = Harness::new(Metadata::Enabled);
    let mut request = harness.rename_request("a", "B");
    request.trainer_loading_mode = "bogus".to_string();

    let error = harness.service.rename(request).await.unwrap_err();
    assert_eq!(error.message(), "invalid trainer loading mode");
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "trainer_loading_mode"
        }
    );
}

#[tokio::test]
async fn rename_refuses_a_traversal_old_slug_before_any_file_operation() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .export(harness.request("Victim"))
        .await
        .unwrap();
    // A decoy that "../victim" would reach from the launchers directory.
    let decoy = harness
        .home
        .join(".local/share/crosshook/victim-trainer.sh");
    fs::write(&decoy, "#!/bin/sh\n# CrossHook watermark decoy\n").unwrap();

    for slug in ["../victim", "..", "a/b", "..\\x", "", "  "] {
        let error = harness
            .service
            .rename(harness.rename_request(slug, "Renamed"))
            .await
            .unwrap_err();
        assert_eq!(
            error.kind(),
            AppErrorKind::Validation {
                field: "old_launcher_slug"
            },
            "slug {slug:?}"
        );
    }
    assert!(decoy.exists());
    assert!(harness.script_path("victim").exists());
    assert!(!harness.script_path("renamed").exists());
}

#[tokio::test]
async fn rename_refuses_old_paths_resolving_outside_the_launcher_dirs() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .export(harness.request("Keep"))
        .await
        .unwrap();

    // Slugs are sanitized upstream; the guard is the last line of defence.
    let escaped =
        super::super::export::ensure_inside_launcher_dirs(&harness.home_str(), "", &["../escape"])
            .unwrap_err();
    assert_eq!(
        escaped.kind(),
        AppErrorKind::Validation {
            field: "old_launcher_slug"
        }
    );
    assert_eq!(
        escaped.message(),
        "launcher paths must stay inside the launcher directories"
    );

    super::super::export::ensure_inside_launcher_dirs(&harness.home_str(), "", &["keep"])
        .expect("plain slug stays inside");
    assert!(harness.script_path("keep").exists());
}

#[tokio::test]
async fn rename_leaves_a_foreign_destination_untouched() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .export(harness.request("Mine"))
        .await
        .unwrap();
    fs::write(
        harness.script_path("taken"),
        "#!/bin/sh\necho not crosshook\n",
    )
    .unwrap();

    let result = harness
        .service
        .rename(harness.rename_request("mine", "Taken"))
        .await
        .unwrap();

    assert!(!result.script_renamed);
    assert!(result.old_script_cleanup_warning.is_some());
    assert!(harness.script_path("mine").exists());
    assert_eq!(
        fs::read_to_string(harness.script_path("taken")).unwrap(),
        "#!/bin/sh\necho not crosshook\n"
    );
}
