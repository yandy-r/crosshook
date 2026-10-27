//! `LauncherExportService` tests: list, orphans and re-export slice.

use std::fs;

use crosshook_core::settings::UmuPreference;

use super::helpers::{Harness, Metadata};
use crate::AppErrorKind;

#[tokio::test]
async fn list_returns_sorted_launchers_with_unknown_slugs_not_stale() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .export(harness.request("Zeta"))
        .await
        .unwrap();
    harness
        .service
        .export(harness.request("Alpha"))
        .await
        .unwrap();

    let listed = harness
        .service
        .list(harness.home_str(), String::new())
        .await;
    let slugs: Vec<_> = listed.iter().map(|l| l.launcher_slug.as_str()).collect();
    assert_eq!(slugs, ["alpha", "zeta"]);
    assert!(
        listed.iter().all(|l| !l.is_stale),
        "no owning profile -> defaults false"
    );
}

#[tokio::test]
async fn list_flags_stale_launchers_from_profile_state() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("Alpha", "Alpha Launcher");
    harness.reexport("alpha-launcher").await;

    let fresh = harness
        .service
        .list(harness.home_str(), String::new())
        .await;
    assert!(!fresh[0].is_stale);

    // Change what the profile would generate; the on-disk script is now stale.
    let mut changed = harness.profiles.load("Alpha").unwrap();
    changed.launch.network_isolation = !changed.launch.network_isolation;
    harness.profiles.save("Alpha", &changed).unwrap();
    let stale = harness
        .service
        .list(harness.home_str(), String::new())
        .await;
    assert_eq!(stale[0].launcher_slug, "alpha-launcher");
    assert!(stale[0].is_stale);
}

#[tokio::test]
async fn list_ors_stale_flags_across_profiles_sharing_a_slug() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("First", "Shared");
    harness.save_profile("Second", "Shared");
    harness.reexport("shared").await;

    let mut second = harness.profiles.load("Second").unwrap();
    second.launch.network_isolation = !second.launch.network_isolation;
    harness.profiles.save("Second", &second).unwrap();

    let listed = harness
        .service
        .list(harness.home_str(), String::new())
        .await;
    assert_eq!(listed.len(), 1);
    assert!(listed[0].is_stale, "any stale owner marks the slug stale");
}

#[tokio::test]
async fn list_survives_settings_failure_and_unloadable_profiles() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("Good", "Good Launcher");
    harness.reexport("good-launcher").await;
    fs::write(harness.profiles.base_path.join("Broken.toml"), "game = [").unwrap();
    harness.corrupt_settings();

    let listed = harness
        .service
        .list(harness.home_str(), String::new())
        .await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].launcher_slug, "good-launcher");
    assert!(!listed[0].is_stale);
}

#[tokio::test]
async fn find_orphaned_filters_known_profile_slugs() {
    let harness = Harness::new(Metadata::Disabled);
    for name in ["Known", "Lonely"] {
        harness.service.export(harness.request(name)).await.unwrap();
    }

    let orphans = harness
        .service
        .find_orphaned(vec!["known".to_string()], harness.home_str(), String::new())
        .await;
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].launcher_slug, "lonely");

    let all = harness
        .service
        .find_orphaned(Vec::new(), harness.home_str(), String::new())
        .await;
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn find_orphaned_skips_symlinked_scripts() {
    let harness = Harness::new(Metadata::Disabled);
    fs::create_dir_all(harness.launchers_dir()).unwrap();
    let target = harness.home.join("real.sh");
    fs::write(&target, "#!/bin/sh\n").unwrap();
    std::os::unix::fs::symlink(&target, harness.script_path("linked")).unwrap();

    let orphans = harness
        .service
        .find_orphaned(Vec::new(), harness.home_str(), String::new())
        .await;
    assert!(orphans.is_empty());
}

#[tokio::test]
async fn reexport_by_slug_rewrites_the_launcher_from_the_owning_profile() {
    let harness = Harness::new(Metadata::Enabled);
    let profile = harness.save_profile("Alpha", "Alpha Launcher");
    let request = harness.request_for_profile(&profile, "Alpha");
    harness.service.export(request).await.unwrap();
    fs::write(harness.script_path("alpha-launcher"), "tampered").unwrap();

    let result = harness
        .service
        .reexport_by_slug(
            "alpha-launcher".to_string(),
            harness.home_str(),
            String::new(),
        )
        .await
        .unwrap();

    assert_eq!(result.launcher_slug, "alpha-launcher");
    let rewritten = fs::read_to_string(harness.script_path("alpha-launcher")).unwrap();
    assert_ne!(rewritten, "tampered");
    assert_eq!(harness.launcher_rows("alpha-launcher", "aligned"), 1);
}

#[tokio::test]
async fn reexport_by_slug_reports_unknown_slug() {
    let harness = Harness::new(Metadata::Enabled);
    harness.save_profile("Alpha", "Alpha Launcher");

    let error = harness
        .service
        .reexport_by_slug("nope".to_string(), harness.home_str(), String::new())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(error.message(), "No profile found for launcher slug 'nope'");
}

#[tokio::test]
async fn reexport_by_slug_hard_fails_on_final_settings_reload() {
    let harness = Harness::new(Metadata::Enabled);
    let profile = harness.save_profile("Alpha", "Alpha Launcher");
    harness
        .service
        .export(harness.request_for_profile(&profile, "Alpha"))
        .await
        .unwrap();
    harness.corrupt_settings();

    // Slug matching falls back to Auto and finds the profile; the final reload errors.
    let error = harness
        .service
        .reexport_by_slug(
            "alpha-launcher".to_string(),
            harness.home_str(),
            String::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "settings" });
}

#[tokio::test]
async fn reexport_by_slug_uses_the_global_umu_preference() {
    let harness = Harness::new(Metadata::Enabled);
    let profile = harness.save_profile("Alpha", "Alpha Launcher");
    harness
        .service
        .export(harness.request_for_profile(&profile, "Alpha"))
        .await
        .unwrap();
    let before = fs::read_to_string(harness.script_path("alpha-launcher")).unwrap();

    let mut settings = crosshook_core::settings::AppSettingsData::default();
    settings.umu_preference = UmuPreference::Proton;
    harness.write_settings(&settings);
    harness
        .service
        .reexport_by_slug(
            "alpha-launcher".to_string(),
            harness.home_str(),
            String::new(),
        )
        .await
        .unwrap();
    let after = fs::read_to_string(harness.script_path("alpha-launcher")).unwrap();
    assert_ne!(
        before, after,
        "umu preference flows into the regenerated script"
    );
}
