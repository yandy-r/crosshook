//! `CollectionsService` tests: TOML export and import preview.

use crosshook_core::profile::{CollectionDefaultsSection, GameProfile};

use super::helpers::Harness;
use crate::AppErrorKind;

fn path_string(h: &Harness, relative: &str) -> String {
    h.path(relative).to_string_lossy().into_owned()
}

#[tokio::test]
async fn export_validates_ids_and_paths() {
    let h = Harness::new();

    let err = h
        .service
        .export_to_toml("   ".to_string(), "/tmp/x.toml".to_string())
        .await
        .unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "collection_id"
        }
    );
    assert_eq!(err.message(), "collection_id must not be empty");

    let err = h
        .service
        .export_to_toml("id".to_string(), String::new())
        .await
        .unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "output_path"
        }
    );
    assert_eq!(err.message(), "output_path must not be empty");

    let err = h.service.import_from_toml(String::new()).await.unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Validation { field: "path" });
    assert_eq!(err.message(), "path must not be empty");
}

#[tokio::test]
async fn export_unknown_collection_reports_not_found_text() {
    let h = Harness::new();
    let err = h
        .service
        .export_to_toml("ghost".to_string(), path_string(&h, "out/ghost.toml"))
        .await
        .unwrap_err();
    assert!(
        err.message().contains("collection not found: ghost"),
        "unexpected message: {}",
        err.message()
    );
    assert!(!h.path("out/ghost.toml").exists());
}

#[tokio::test]
async fn export_then_import_preview_roundtrips_without_mutating() {
    let h = Harness::new();
    let mut profile = GameProfile::default();
    profile.game.name = "Elden Ring".to_string();
    profile.steam.app_id = "1245620".to_string();
    h.register_profile("elden-ring", &profile);

    let defaults = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    let id = h.collection_with_defaults("Action", &defaults);
    h.service
        .add_profile(id.clone(), "elden-ring".to_string())
        .await
        .unwrap();

    let out = path_string(&h, "exports/action.toml");
    // The collection id is trimmed before use.
    let exported = h
        .service
        .export_to_toml(format!("  {id}  "), out.clone())
        .await
        .unwrap();
    assert_eq!(exported.collection_id, id);
    assert_eq!(exported.manifest.name, "Action");
    assert_eq!(exported.manifest.profiles.len(), 1);
    assert!(h.path("exports/action.toml").is_file());

    let collections_before = h.service.list().await.unwrap().len();
    let preview = h.service.import_from_toml(out).await.unwrap();
    assert_eq!(preview.matched.len(), 1);
    assert_eq!(preview.matched[0].local_profile_name, "elden-ring");
    assert!(preview.ambiguous.is_empty());
    assert!(preview.unmatched.is_empty());
    assert_eq!(
        preview
            .manifest
            .defaults
            .as_ref()
            .and_then(|d| d.method.as_deref()),
        Some("proton_run")
    );
    // Preview is non-mutating: no collection was created.
    assert_eq!(h.service.list().await.unwrap().len(), collections_before);
}

#[tokio::test]
async fn export_with_missing_member_profile_file_is_an_error() {
    let h = Harness::new();
    h.register_named("ghosted");
    let id = h.service.create("Broken".to_string()).await.unwrap();
    h.service
        .add_profile(id.clone(), "ghosted".to_string())
        .await
        .unwrap();
    std::fs::remove_file(h.path("profiles/ghosted.toml")).unwrap();

    let err = h
        .service
        .export_to_toml(id, path_string(&h, "out.toml"))
        .await
        .unwrap_err();

    assert!(err.message().contains("is missing"), "{}", err.message());
}

#[tokio::test]
async fn import_missing_and_malformed_files_error() {
    let h = Harness::new();

    let err = h
        .service
        .import_from_toml(path_string(&h, "missing.toml"))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Io);

    std::fs::write(h.path("bad.toml"), "not toml [[[").unwrap();
    let err = h
        .service
        .import_from_toml(path_string(&h, "bad.toml"))
        .await
        .unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "collection_preset"
        }
    );

    std::fs::write(
        h.path("future.toml"),
        "schema_version = \"2\"\nname = \"X\"\nprofiles = []\n",
    )
    .unwrap();
    let err = h
        .service
        .import_from_toml(path_string(&h, "future.toml"))
        .await
        .unwrap_err();
    assert!(err
        .message()
        .contains("unsupported collection preset schema version"));
}

#[tokio::test]
async fn import_works_with_disabled_metadata() {
    let h = Harness::disabled();
    std::fs::write(
        h.path("preset.toml"),
        "schema_version = \"1\"\nname = \"X\"\nprofiles = []\n",
    )
    .unwrap();

    let preview = h
        .service
        .import_from_toml(path_string(&h, "preset.toml"))
        .await
        .unwrap();

    assert!(preview.matched.is_empty() && preview.unmatched.is_empty());
}

#[tokio::test]
async fn export_with_disabled_metadata_is_unavailable() {
    let h = Harness::disabled();
    let err = h
        .service
        .export_to_toml("id".to_string(), path_string(&h, "o.toml"))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Unavailable);
}
