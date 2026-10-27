//! Coexistence analysis through the service.

use crosshook_core::mods::{ModCategory, ProfileModInput};

use super::helpers::{mod_input, Harness, Metadata};
use crate::AppErrorKind;

fn codes(issues: &[crosshook_core::launch::LaunchValidationIssue]) -> Vec<&str> {
    issues.iter().filter_map(|i| i.code.as_deref()).collect()
}

#[tokio::test]
async fn analyze_with_disabled_metadata_is_unavailable() {
    let harness = Harness::new(Metadata::Disabled);
    harness.seed_profile("alpha", &harness.game_exe());

    let error = harness.service.analyze("alpha".into()).await.unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Unavailable);
}

#[tokio::test]
async fn analyze_for_unregistered_profile_reports_missing_identity() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile_unregistered("alpha", &harness.game_exe());

    let error = harness.service.analyze("alpha".into()).await.unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(
        error.message(),
        "profile has no metadata record yet — save the profile once, then retry"
    );
}

#[tokio::test]
async fn analyze_without_registered_mods_has_no_issues() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());

    let issues = harness.service.analyze("alpha".into()).await.unwrap();

    assert!(issues.is_empty());
}

#[tokio::test]
async fn analyze_passes_core_issues_through() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    harness
        .service
        .add(
            "alpha".into(),
            ProfileModInput {
                category: ModCategory::FileReplacement,
                paths: vec!["Data/missing-texture.dds".into()],
                ..mod_input("Texture pack")
            },
        )
        .await
        .unwrap();

    let issues = harness.service.analyze("alpha".into()).await.unwrap();

    let codes = codes(&issues);
    assert!(codes.contains(&"mod_coexistence_file_replacement_notice"));
    assert!(codes.contains(&"mod_coexistence_registered_path_missing"));
}

#[tokio::test]
async fn analyze_ignores_disabled_mods_and_other_profiles() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    harness.seed_profile("beta", &harness.game_exe());
    harness
        .service
        .add(
            "alpha".into(),
            ProfileModInput {
                category: ModCategory::FileReplacement,
                enabled: false,
                ..mod_input("Disabled pack")
            },
        )
        .await
        .unwrap();
    harness
        .service
        .add(
            "beta".into(),
            ProfileModInput {
                category: ModCategory::FileReplacement,
                ..mod_input("Other profile pack")
            },
        )
        .await
        .unwrap();

    let issues = harness.service.analyze("alpha".into()).await.unwrap();

    assert!(issues.is_empty());
}
