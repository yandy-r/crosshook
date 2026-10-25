//! Favorite purity and rename event tests: a favorite toggle publishes only
//! `FavoriteToggled` with the concrete name and leaves the TOML, config
//! revisions and cached health untouched; rename publishes `Renamed { from }`.

use std::fs;

use super::helpers::{steam_profile, Harness, Metadata};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

#[tokio::test]
async fn favorite_publishes_only_favorite_toggled_and_touches_no_profile_state() {
    let mut harness = Harness::new(Metadata::Enabled);
    let home = harness.root.path().to_string_lossy().into_owned();
    harness
        .writer
        .save("Alpha".to_string(), steam_profile(&home))
        .await
        .unwrap();
    assert!(harness.next_event().await.is_some());
    let profile_id = harness
        .metadata
        .lookup_profile_id("Alpha")
        .unwrap()
        .unwrap();
    harness
        .metadata
        .upsert_health_snapshot(&profile_id, "healthy", 0, "2026-01-01T00:00:00Z")
        .unwrap();
    let toml_path = harness.profiles.base_path.join("Alpha.toml");
    let toml_before = fs::read(&toml_path).unwrap();
    let revisions_before = harness
        .metadata
        .list_config_revisions(&profile_id, None)
        .unwrap()
        .len();

    let commit = harness
        .writer
        .set_favorite("Alpha".to_string(), true)
        .await
        .unwrap();

    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    assert_eq!(commit.new_revision_id, None);
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::FavoriteToggled,
            names: vec!["Alpha".to_string()],
        })
    );
    assert_eq!(harness.next_event().await, None);
    assert_eq!(
        harness.metadata.list_favorite_profiles().unwrap(),
        ["Alpha"]
    );
    assert_eq!(fs::read(&toml_path).unwrap(), toml_before);
    assert_eq!(
        harness
            .metadata
            .list_config_revisions(&profile_id, None)
            .unwrap()
            .len(),
        revisions_before
    );
    assert!(harness
        .metadata
        .lookup_health_snapshot(&profile_id)
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn favorite_with_disabled_metadata_is_unavailable_and_silent() {
    let mut harness = Harness::new(Metadata::Disabled);
    let home = harness.root.path().to_string_lossy().into_owned();
    harness
        .profiles
        .save("Alpha", &steam_profile(&home))
        .unwrap();
    let toml_path = harness.profiles.base_path.join("Alpha.toml");
    let toml_before = fs::read(&toml_path).unwrap();

    let error = harness
        .writer
        .set_favorite("Alpha".to_string(), true)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Unavailable);
    assert_eq!(harness.next_event().await, None);
    assert_eq!(fs::read(&toml_path).unwrap(), toml_before);
}

#[tokio::test]
async fn favorite_rejects_invalid_name_before_any_side_effect() {
    let mut harness = Harness::new(Metadata::Enabled);

    let error = harness
        .writer
        .set_favorite("../escape".to_string(), true)
        .await
        .unwrap_err();

    assert!(matches!(
        error.kind(),
        AppErrorKind::Validation { field: "profile" }
    ));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn rename_publishes_renamed_from_old_with_new_name() {
    let mut harness = Harness::new(Metadata::Enabled);
    let home = harness.root.path().to_string_lossy().into_owned();
    harness
        .writer
        .save("Alpha".to_string(), steam_profile(&home))
        .await
        .unwrap();
    assert!(harness.next_event().await.is_some());

    harness
        .writer
        .rename("Alpha".to_string(), "Beta".to_string())
        .await
        .unwrap();

    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Renamed {
                from: "Alpha".to_string(),
            },
            names: vec!["Beta".to_string()],
        })
    );
    assert_eq!(harness.next_event().await, None);
}
