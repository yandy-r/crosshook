//! Registry CRUD: availability guards, identity resolution, error texts and
//! per-profile isolation.

use crosshook_core::mods::{ModCategory, ProfileModInput};

use super::helpers::{mod_input, Harness, Metadata};
use crate::AppErrorKind;

const UNAVAILABLE: &str = "metadata_unavailable: the metadata database could not be opened";
const MISSING_IDENTITY: &str =
    "profile has no metadata record yet — save the profile once, then retry";
const MOD_NOT_FOUND: &str = "metadata validation error: mod not found";
/// `remove` keeps the legacy bare text (no store-error prefix).
const REMOVE_MOD_NOT_FOUND: &str = "mod not found";

fn validation_mod() -> AppErrorKind {
    AppErrorKind::Validation { field: "mod" }
}

#[tokio::test]
async fn list_with_disabled_metadata_reports_unavailable_without_touching_profiles() {
    let harness = Harness::new(Metadata::Disabled);

    let response = harness.service.list("does-not-exist".into()).await.unwrap();

    assert!(!response.available);
    assert!(response.mods.is_empty());
}

#[tokio::test]
async fn list_for_unregistered_profile_is_available_and_empty() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile_unregistered("alpha", &harness.game_exe());

    let response = harness.service.list("alpha".into()).await.unwrap();

    assert!(response.available);
    assert!(response.mods.is_empty());
}

#[tokio::test]
async fn list_for_missing_profile_is_not_found() {
    let harness = Harness::new(Metadata::Enabled);

    let error = harness.service.list("ghost".into()).await.unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::NotFound);
}

#[tokio::test]
async fn add_then_list_returns_the_mod_sorted_by_name() {
    let harness = Harness::new(Metadata::Enabled);
    let profile_id = harness.seed_profile("alpha", &harness.game_exe()).unwrap();

    let zeta = harness
        .service
        .add("alpha".into(), mod_input("Zeta"))
        .await
        .unwrap();
    harness
        .service
        .add("alpha".into(), mod_input("alpha mod"))
        .await
        .unwrap();

    assert_eq!(zeta.profile_id, profile_id);
    let response = harness.service.list("alpha".into()).await.unwrap();
    assert!(response.available);
    let names: Vec<_> = response.mods.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["alpha mod", "Zeta"]);
}

#[tokio::test]
async fn writes_and_analysis_with_disabled_metadata_are_unavailable() {
    let harness = Harness::new(Metadata::Disabled);
    // The guard runs before any profile lookup, so the profile need not exist.
    let add = harness
        .service
        .add("ghost".into(), mod_input("X"))
        .await
        .unwrap_err();
    let update = harness
        .service
        .update("ghost".into(), "id".into(), mod_input("X"))
        .await
        .unwrap_err();
    let remove = harness
        .service
        .remove("ghost".into(), "id".into())
        .await
        .unwrap_err();
    let analyze = harness.service.analyze("ghost".into()).await.unwrap_err();

    for error in [add, update, remove, analyze] {
        assert_eq!(error.kind(), AppErrorKind::Unavailable);
        assert_eq!(error.message(), UNAVAILABLE);
    }
}

#[tokio::test]
async fn writes_for_unregistered_profile_report_missing_identity() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile_unregistered("alpha", &harness.game_exe());

    let add = harness
        .service
        .add("alpha".into(), mod_input("X"))
        .await
        .unwrap_err();
    let update = harness
        .service
        .update("alpha".into(), "id".into(), mod_input("X"))
        .await
        .unwrap_err();
    let remove = harness
        .service
        .remove("alpha".into(), "id".into())
        .await
        .unwrap_err();

    for error in [add, update, remove] {
        assert_eq!(error.kind(), AppErrorKind::NotFound);
        assert_eq!(error.message(), MISSING_IDENTITY);
    }
}

#[tokio::test]
async fn add_for_missing_profile_is_not_found() {
    let harness = Harness::new(Metadata::Enabled);

    let error = harness
        .service
        .add("ghost".into(), mod_input("X"))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::NotFound);
}

#[tokio::test]
async fn duplicate_name_is_a_mod_validation_error_with_the_store_text() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    harness
        .service
        .add("alpha".into(), mod_input("ReShade"))
        .await
        .unwrap();

    let error = harness
        .service
        .add("alpha".into(), mod_input("ReShade"))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), validation_mod());
    assert_eq!(
        error.message(),
        "metadata validation error: a mod named “ReShade” is already registered for this profile"
    );
}

#[tokio::test]
async fn blank_name_is_rejected_with_the_store_text() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());

    let error = harness
        .service
        .add("alpha".into(), mod_input("   "))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), validation_mod());
    assert_eq!(
        error.message(),
        "metadata validation error: mod name is required"
    );
}

#[tokio::test]
async fn update_replaces_the_registered_fields() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    let added = harness
        .service
        .add("alpha".into(), mod_input("Old"))
        .await
        .unwrap();

    let updated = harness
        .service
        .update(
            "alpha".into(),
            added.mod_id.clone(),
            ProfileModInput {
                category: ModCategory::ScriptExtender,
                paths: vec!["skse64_loader.exe".into()],
                enabled: false,
                ..mod_input("New")
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.mod_id, added.mod_id);
    assert_eq!(updated.name, "New");
    assert_eq!(updated.category, ModCategory::ScriptExtender);
    assert_eq!(updated.paths, ["skse64_loader.exe"]);
    assert!(!updated.enabled);
    let listed = harness.service.list("alpha".into()).await.unwrap().mods;
    assert_eq!(listed, [updated]);
}

#[tokio::test]
async fn update_of_an_unknown_mod_id_is_mod_not_found() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());

    let error = harness
        .service
        .update("alpha".into(), "stale".into(), mod_input("X"))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), validation_mod());
    assert_eq!(error.message(), MOD_NOT_FOUND);
}

#[tokio::test]
async fn remove_deletes_the_mod_and_a_stale_remove_is_mod_not_found() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    let added = harness
        .service
        .add("alpha".into(), mod_input("ReShade"))
        .await
        .unwrap();

    harness
        .service
        .remove("alpha".into(), added.mod_id.clone())
        .await
        .unwrap();
    let stale = harness
        .service
        .remove("alpha".into(), added.mod_id)
        .await
        .unwrap_err();

    assert!(harness
        .service
        .list("alpha".into())
        .await
        .unwrap()
        .mods
        .is_empty());
    assert_eq!(stale.kind(), AppErrorKind::NotFound);
    assert_eq!(stale.message(), REMOVE_MOD_NOT_FOUND);
}

#[tokio::test]
async fn remove_never_touches_files_on_disk() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    harness.touch_game_file("ReShade.ini");
    let added = harness
        .service
        .add(
            "alpha".into(),
            ProfileModInput {
                paths: vec!["ReShade.ini".into()],
                ..mod_input("ReShade")
            },
        )
        .await
        .unwrap();

    harness
        .service
        .remove("alpha".into(), added.mod_id)
        .await
        .unwrap();

    assert!(harness.game_dir().join("ReShade.ini").exists());
}

#[tokio::test]
async fn mod_ids_are_isolated_between_profiles() {
    let harness = Harness::new(Metadata::Enabled);
    harness.seed_profile("alpha", &harness.game_exe());
    harness.seed_profile("beta", &harness.game_exe());
    let alpha_mod = harness
        .service
        .add("alpha".into(), mod_input("Shared name"))
        .await
        .unwrap();

    let update = harness
        .service
        .update("beta".into(), alpha_mod.mod_id.clone(), mod_input("Hijack"))
        .await
        .unwrap_err();
    let remove = harness
        .service
        .remove("beta".into(), alpha_mod.mod_id.clone())
        .await
        .unwrap_err();

    assert_eq!(update.message(), MOD_NOT_FOUND);
    assert_eq!(remove.message(), REMOVE_MOD_NOT_FOUND);
    let alpha = harness.service.list("alpha".into()).await.unwrap().mods;
    assert_eq!(alpha, [alpha_mod]);
    assert!(harness
        .service
        .list("beta".into())
        .await
        .unwrap()
        .mods
        .is_empty());
    // The same mod name is allowed on another profile.
    harness
        .service
        .add("beta".into(), mod_input("Shared name"))
        .await
        .unwrap();
}
