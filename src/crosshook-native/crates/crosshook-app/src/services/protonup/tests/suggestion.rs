//! `get_suggestion`: a community Proton requirement compared with the
//! runtimes discovered under the Steam client root.

use crosshook_core::protonup::ProtonUpMatchStatus;

use super::helpers::{Harness, Mode};

#[tokio::test]
async fn blank_community_version_is_unknown() {
    let harness = Harness::new(Mode::Succeed);
    let steam = harness.steam_root();

    let suggestion = harness
        .service
        .get_suggestion(
            "   ".to_string(),
            Some(steam.to_string_lossy().into_owned()),
        )
        .await
        .unwrap();

    assert_eq!(suggestion.status, ProtonUpMatchStatus::Unknown);
    assert!(suggestion.community_version.is_none());
}

#[tokio::test]
async fn installed_runtime_matches_its_community_version_case_insensitively() {
    let harness = Harness::new(Mode::Succeed);
    harness.make_tool("GE-Proton9-20");
    let steam = harness.steam_root();

    let suggestion = harness
        .service
        .get_suggestion(
            "ge-proton9-20".to_string(),
            Some(steam.to_string_lossy().into_owned()),
        )
        .await
        .unwrap();

    assert_eq!(suggestion.status, ProtonUpMatchStatus::Matched);
    assert_eq!(
        suggestion.matched_install_name.as_deref(),
        Some("GE-Proton9-20")
    );
}

#[tokio::test]
async fn missing_runtime_is_not_matched() {
    let harness = Harness::new(Mode::Succeed);
    let steam = harness.steam_root();

    let suggestion = harness
        .service
        .get_suggestion(
            "GE-Proton9-20".to_string(),
            Some(steam.to_string_lossy().into_owned()),
        )
        .await
        .unwrap();

    assert_ne!(suggestion.status, ProtonUpMatchStatus::Matched);
    assert!(suggestion.matched_install_name.is_none());
}
