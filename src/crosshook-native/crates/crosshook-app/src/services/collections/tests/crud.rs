//! `CollectionsService` tests: crud slice.

use super::helpers::Harness;
use crate::AppErrorKind;

#[tokio::test]
async fn create_list_rename_and_describe() {
    let h = Harness::new();
    let id = h.service.create("Speedrun".to_string()).await.unwrap();
    assert!(!id.trim().is_empty());

    h.service
        .rename(id.clone(), "Speed".to_string())
        .await
        .unwrap();
    h.service
        .update_description(id.clone(), Some("fast".to_string()))
        .await
        .unwrap();

    let rows = h.service.list().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].collection_id, id);
    assert_eq!(rows[0].name, "Speed");
    assert_eq!(rows[0].description.as_deref(), Some("fast"));
    assert_eq!(rows[0].profile_count, 0);

    h.service.update_description(id, None).await.unwrap();
    assert_eq!(h.service.list().await.unwrap()[0].description, None);
}

#[tokio::test]
async fn create_empty_name_is_validation_error() {
    let h = Harness::new();
    let err = h.service.create("   ".to_string()).await.unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "collection"
        }
    );
    assert_eq!(
        err.message(),
        "metadata validation error: collection name must not be empty"
    );
}

#[tokio::test]
async fn duplicate_name_surfaces_as_error() {
    let h = Harness::new();
    h.service.create("Dup".to_string()).await.unwrap();
    let err = h.service.create("Dup".to_string()).await.unwrap_err();
    assert_ne!(err.kind(), AppErrorKind::Unavailable);
}

#[tokio::test]
async fn rename_unknown_id_keeps_not_found_text() {
    let h = Harness::new();
    let err = h
        .service
        .rename("no-such-id".to_string(), "X".to_string())
        .await
        .unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "collection"
        }
    );
    assert_eq!(
        err.message(),
        "metadata validation error: collection not found: no-such-id"
    );
}

#[tokio::test]
async fn add_profile_missing_name_keeps_text() {
    let h = Harness::new();
    let id = h.service.create("C".to_string()).await.unwrap();
    let err = h
        .service
        .add_profile(id, "ghost".to_string())
        .await
        .unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "collection"
        }
    );
    assert_eq!(
        err.message(),
        "metadata validation error: profile not found when adding to collection: ghost"
    );
}

#[tokio::test]
async fn membership_add_is_idempotent_and_removal_is_scoped() {
    let h = Harness::new();
    h.register_named("alpha");
    h.register_named("beta");
    let id = h.service.create("C".to_string()).await.unwrap();

    h.service
        .add_profile(id.clone(), "alpha".to_string())
        .await
        .unwrap();
    h.service
        .add_profile(id.clone(), "alpha".to_string())
        .await
        .unwrap();
    h.service
        .add_profile(id.clone(), "beta".to_string())
        .await
        .unwrap();

    let mut members = h.service.list_profiles(id.clone()).await.unwrap();
    members.sort();
    assert_eq!(members, vec!["alpha", "beta"]);
    assert_eq!(h.service.list().await.unwrap()[0].profile_count, 2);

    let for_alpha = h
        .service
        .collections_for_profile("alpha".to_string())
        .await
        .unwrap();
    assert_eq!(for_alpha.len(), 1);
    assert_eq!(for_alpha[0].collection_id, id);

    h.service
        .remove_profile(id.clone(), "alpha".to_string())
        .await
        .unwrap();
    // Removing an absent or unregistered profile is a no-op.
    h.service
        .remove_profile(id.clone(), "alpha".to_string())
        .await
        .unwrap();
    h.service
        .remove_profile(id.clone(), "ghost".to_string())
        .await
        .unwrap();
    assert_eq!(h.service.list_profiles(id).await.unwrap(), vec!["beta"]);
    assert!(h
        .service
        .collections_for_profile("unknown".to_string())
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn delete_cascades_memberships_and_unknown_id_is_ok() {
    let h = Harness::new();
    h.register_named("alpha");
    let id = h.service.create("C".to_string()).await.unwrap();
    h.service
        .add_profile(id.clone(), "alpha".to_string())
        .await
        .unwrap();

    h.service.delete(id.clone()).await.unwrap();
    assert!(h.service.list().await.unwrap().is_empty());
    assert!(h
        .service
        .collections_for_profile("alpha".to_string())
        .await
        .unwrap()
        .is_empty());
    h.service.delete(id).await.unwrap();
    h.service.delete("never-existed".to_string()).await.unwrap();
}

#[tokio::test]
async fn disabled_store_guards_writes_but_lists_stay_empty() {
    let h = Harness::disabled();

    assert!(h.service.list().await.unwrap().is_empty());
    assert!(h
        .service
        .collections_for_profile("alpha".to_string())
        .await
        .unwrap()
        .is_empty());

    let unavailable = |kind: AppErrorKind| assert_eq!(kind, AppErrorKind::Unavailable);
    unavailable(h.service.create("X".to_string()).await.unwrap_err().kind());
    unavailable(h.service.delete("id".to_string()).await.unwrap_err().kind());
    unavailable(
        h.service
            .add_profile("id".to_string(), "p".to_string())
            .await
            .unwrap_err()
            .kind(),
    );
    unavailable(
        h.service
            .remove_profile("id".to_string(), "p".to_string())
            .await
            .unwrap_err()
            .kind(),
    );
    unavailable(
        h.service
            .list_profiles("id".to_string())
            .await
            .unwrap_err()
            .kind(),
    );
    unavailable(
        h.service
            .rename("id".to_string(), "n".to_string())
            .await
            .unwrap_err()
            .kind(),
    );
    unavailable(
        h.service
            .update_description("id".to_string(), None)
            .await
            .unwrap_err()
            .kind(),
    );
}
