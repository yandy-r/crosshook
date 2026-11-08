//! Read-side facades behind the favorites and config-history commands:
//! `list_favorites`, `config_history` and `mark_known_good`.

use super::helpers::{steam_profile, Harness, Metadata};
use crate::AppErrorKind;

#[tokio::test]
async fn list_favorites_follows_set_favorite_and_is_empty_without_metadata() {
    let harness = Harness::new(Metadata::Enabled);
    let home = harness.root.path().to_string_lossy().into_owned();
    harness
        .writer
        .save("Alpha".to_string(), steam_profile(&home))
        .await
        .unwrap();
    assert!(harness.writer.list_favorites().await.unwrap().is_empty());

    harness
        .writer
        .set_favorite("Alpha".to_string(), true)
        .await
        .unwrap();
    assert_eq!(harness.writer.list_favorites().await.unwrap(), ["Alpha"]);

    harness
        .writer
        .set_favorite("Alpha".to_string(), false)
        .await
        .unwrap();
    assert!(harness.writer.list_favorites().await.unwrap().is_empty());

    let disabled = Harness::new(Metadata::Disabled);
    assert!(disabled.writer.list_favorites().await.unwrap().is_empty());
}

#[tokio::test]
async fn config_history_lists_saved_revisions_newest_first_and_honours_the_limit() {
    let harness = Harness::new(Metadata::Enabled);
    let home = harness.root.path().to_string_lossy().into_owned();
    let mut profile = steam_profile(&home);
    harness
        .writer
        .save("Alpha".to_string(), profile.clone())
        .await
        .unwrap();
    profile
        .launch
        .custom_env_vars
        .insert("A".into(), "1".into());
    harness
        .writer
        .save("Alpha".to_string(), profile)
        .await
        .unwrap();

    let all = harness
        .writer
        .config_history("Alpha".to_string(), None)
        .await
        .unwrap();
    let one = harness
        .writer
        .config_history("Alpha".to_string(), Some(1))
        .await
        .unwrap();

    assert_eq!(all.len(), 2);
    assert!(all[0].id > all[1].id, "newest first");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, all[0].id);
    assert!(harness
        .writer
        .config_history("Unknown".to_string(), None)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn config_history_is_unavailable_without_metadata() {
    let harness = Harness::new(Metadata::Disabled);

    let error = harness
        .writer
        .config_history("Alpha".to_string(), None)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Unavailable);
}

#[tokio::test]
async fn mark_known_good_validates_the_revision_and_the_profile() {
    let harness = Harness::new(Metadata::Enabled);
    let home = harness.root.path().to_string_lossy().into_owned();
    harness
        .writer
        .save("Alpha".to_string(), steam_profile(&home))
        .await
        .unwrap();
    let revision = harness
        .writer
        .config_history("Alpha".to_string(), None)
        .await
        .unwrap()[0]
        .id;

    harness
        .writer
        .mark_known_good("Alpha".to_string(), revision)
        .await
        .unwrap();
    let marked = harness
        .writer
        .config_history("Alpha".to_string(), None)
        .await
        .unwrap();
    assert!(marked
        .iter()
        .any(|row| row.id == revision && row.is_last_known_working));

    let non_positive = harness
        .writer
        .mark_known_good("Alpha".to_string(), 0)
        .await
        .unwrap_err();
    assert_eq!(
        non_positive.kind(),
        AppErrorKind::Validation {
            field: "revision_id"
        }
    );
    let foreign = harness
        .writer
        .mark_known_good("Alpha".to_string(), revision + 1000)
        .await
        .unwrap_err();
    assert_eq!(foreign.kind(), AppErrorKind::NotFound);
}
