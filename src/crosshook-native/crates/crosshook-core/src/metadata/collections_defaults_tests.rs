#![cfg(test)]

use rusqlite::params;

use super::test_support::connection;
use super::{MetadataStore, MetadataStoreError};
use crate::profile::CollectionDefaultsSection;

#[test]
fn test_collection_defaults_set_and_get_roundtrip() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Steam Deck").unwrap();

    // Initially, no defaults.
    let none = store.get_collection_defaults(&id).unwrap();
    assert!(none.is_none());

    let mut defaults = CollectionDefaultsSection::default();
    defaults.method = Some("proton_run".to_string());
    defaults
        .custom_env_vars
        .insert("DXVK_HUD".to_string(), "1".to_string());
    defaults.network_isolation = Some(false);

    store.set_collection_defaults(&id, Some(&defaults)).unwrap();

    let loaded = store
        .get_collection_defaults(&id)
        .unwrap()
        .expect("defaults should be set");
    assert_eq!(loaded.method.as_deref(), Some("proton_run"));
    assert_eq!(loaded.network_isolation, Some(false));
    assert_eq!(
        loaded.custom_env_vars.get("DXVK_HUD").cloned(),
        Some("1".to_string())
    );
}

#[test]
fn test_collection_defaults_clear_writes_null() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Temp").unwrap();

    let mut defaults = CollectionDefaultsSection::default();
    defaults.method = Some("native".to_string());
    store.set_collection_defaults(&id, Some(&defaults)).unwrap();

    // Clearing via None writes NULL.
    store.set_collection_defaults(&id, None).unwrap();
    assert!(store.get_collection_defaults(&id).unwrap().is_none());

    // Clearing via empty-defaults struct ALSO writes NULL (is_empty() guard).
    store
        .set_collection_defaults(&id, Some(&CollectionDefaultsSection::default()))
        .unwrap();
    assert!(
        store.get_collection_defaults(&id).unwrap().is_none(),
        "empty defaults should normalize to NULL"
    );
}

#[test]
fn test_collection_defaults_unknown_id_errors_on_set() {
    let store = MetadataStore::open_in_memory().unwrap();
    let mut defaults = CollectionDefaultsSection::default();
    defaults.method = Some("native".to_string());
    let result = store.set_collection_defaults("no-such-id", Some(&defaults));
    assert!(matches!(result, Err(MetadataStoreError::Validation(_))));
}

#[test]
fn test_collection_defaults_corrupt_json_returns_corrupt_error() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Corrupt").unwrap();

    // Force a corrupt JSON payload via raw SQL.
    let conn = connection(&store);
    conn.execute(
        "UPDATE collections SET defaults_json = ?1 WHERE collection_id = ?2",
        params!["{not-valid-json", id],
    )
    .unwrap();
    drop(conn);

    let result = store.get_collection_defaults(&id);
    assert!(
        matches!(result, Err(MetadataStoreError::Corrupt(_))),
        "corrupt JSON should surface as Corrupt, got {result:?}"
    );
}

#[test]
fn test_collection_defaults_cascades_on_collection_delete() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Scratch").unwrap();

    let mut defaults = CollectionDefaultsSection::default();
    defaults.method = Some("native".to_string());
    store.set_collection_defaults(&id, Some(&defaults)).unwrap();

    store.delete_collection(&id).unwrap();

    // After delete, reading defaults should error because the collection row is gone.
    // The error shape must match `set_collection_defaults` (Validation) so frontend
    // code sees a single surface for the missing-collection condition.
    let result = store.get_collection_defaults(&id);
    assert!(
        matches!(result, Err(MetadataStoreError::Validation(_))),
        "deleted collection defaults read should return Validation, got {result:?}"
    );
}
