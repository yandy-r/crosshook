//! `CollectionsService` sync-resolver and get/set defaults tests (split
//! from `defaults_merge`).

use crosshook_core::profile::CollectionDefaultsSection;

use super::helpers::Harness;
use crate::AppErrorKind;

fn extra_table(key: &str, value: &str) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert(key.to_string(), toml::Value::String(value.to_string()));
    table
}

// ── resolve_defaults_sync branches (AC2) ────────────────────────────────

#[test]
fn sync_resolver_none_and_blank_ids_yield_none() {
    let h = Harness::new();
    assert_eq!(h.service.resolve_defaults_sync(None).unwrap(), None);
    assert_eq!(h.service.resolve_defaults_sync(Some("  ")).unwrap(), None);
}

#[test]
fn sync_resolver_returns_stored_defaults() {
    let h = Harness::new();
    let defaults = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    let id = h.collection_with_defaults("C", &defaults);

    // The id is trimmed before lookup.
    let resolved = h
        .service
        .resolve_defaults_sync(Some(&format!("  {id} ")))
        .unwrap();

    assert_eq!(resolved, Some(defaults));
}

#[test]
fn sync_resolver_unknown_id_and_null_defaults_yield_none() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Empty").unwrap();

    assert_eq!(
        h.service.resolve_defaults_sync(Some("no-such-id")).unwrap(),
        None
    );
    assert_eq!(h.service.resolve_defaults_sync(Some(&id)).unwrap(), None);
}

#[test]
fn sync_resolver_corrupt_defaults_error() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Broken").unwrap();
    h.corrupt_defaults(&id);

    let err = h.service.resolve_defaults_sync(Some(&id)).unwrap_err();

    assert_eq!(err.kind(), AppErrorKind::Internal);
    assert!(
        err.message().starts_with(&format!(
            "metadata database is corrupt: corrupt collection defaults JSON for {id}: "
        )),
        "corrupt message must name the collection, got: {}",
        err.message()
    );
    assert!(
        err.message().contains("key must be a string"),
        "corrupt message must keep the parse detail, got: {}",
        err.message()
    );
}

#[test]
fn sync_resolver_disabled_store_yields_none() {
    let h = Harness::disabled();
    assert_eq!(h.service.resolve_defaults_sync(Some("any")).unwrap(), None);
}

// ── get/set defaults ────────────────────────────────────────────────────

#[tokio::test]
async fn get_defaults_clears_extras() {
    let h = Harness::new();
    let mut seeded = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    seeded.extra = extra_table("future", "x");
    let id = h.collection_with_defaults("C", &seeded);

    let got = h.service.get_defaults(id).await.unwrap().unwrap();

    assert_eq!(got.method.as_deref(), Some("proton_run"));
    assert!(got.extra.is_empty());
}

#[tokio::test]
async fn set_defaults_ignores_incoming_extras_and_preserves_stored_ones() {
    let h = Harness::new();
    let mut seeded = CollectionDefaultsSection {
        method: Some("native".to_string()),
        ..CollectionDefaultsSection::default()
    };
    seeded.extra = extra_table("stored", "keep");
    let id = h.collection_with_defaults("C", &seeded);

    let mut incoming = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    incoming.extra = extra_table("injected", "nope");
    h.service
        .set_defaults(id.clone(), Some(incoming))
        .await
        .unwrap();

    let stored = h.metadata.get_collection_defaults(&id).unwrap().unwrap();
    assert_eq!(stored.method.as_deref(), Some("proton_run"));
    assert_eq!(stored.extra, extra_table("stored", "keep"));
}

#[tokio::test]
async fn set_defaults_none_clears() {
    let h = Harness::new();
    let defaults = CollectionDefaultsSection {
        method: Some("native".to_string()),
        ..CollectionDefaultsSection::default()
    };
    let id = h.collection_with_defaults("C", &defaults);

    h.service.set_defaults(id.clone(), None).await.unwrap();

    assert_eq!(h.service.get_defaults(id).await.unwrap(), None);
}

#[tokio::test]
async fn set_defaults_fails_on_corrupt_existing_row_but_none_clears_it() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Broken").unwrap();
    h.corrupt_defaults(&id);
    let incoming = CollectionDefaultsSection {
        method: Some("native".to_string()),
        ..CollectionDefaultsSection::default()
    };

    let err = h
        .service
        .set_defaults(id.clone(), Some(incoming))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Internal);

    h.service.set_defaults(id.clone(), None).await.unwrap();
    assert_eq!(h.service.get_defaults(id).await.unwrap(), None);
}

#[tokio::test]
async fn defaults_unknown_collection_keeps_not_found_text() {
    let h = Harness::new();
    let err = h
        .service
        .get_defaults("ghost".to_string())
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
        "metadata validation error: collection not found: ghost"
    );
    let err = h
        .service
        .set_defaults("ghost".to_string(), None)
        .await
        .unwrap_err();
    assert_eq!(
        err.message(),
        "metadata validation error: collection not found: ghost"
    );
}

#[tokio::test]
async fn defaults_disabled_store_is_unavailable() {
    let h = Harness::disabled();
    let err = h.service.get_defaults("id".to_string()).await.unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Unavailable);
    let err = h
        .service
        .set_defaults("id".to_string(), None)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Unavailable);
}
