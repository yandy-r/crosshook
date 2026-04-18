#![cfg(test)]

use rusqlite::params;

use super::test_support::{connection, sample_profile};
use super::{MetadataStore, MetadataStoreError, SyncSource};

#[test]
fn test_create_collection_returns_id() {
    let store = MetadataStore::open_in_memory().unwrap();

    let collection_id = store.create_collection("My Favorites").unwrap();
    assert!(!collection_id.trim().is_empty());

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collections WHERE name = ?1",
            params!["My Favorites"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(row_count, 1);
}

#[test]
fn test_add_profile_to_collection() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let collection_id = store.create_collection("Test Collection").unwrap();
    store
        .add_profile_to_collection(&collection_id, "elden-ring")
        .unwrap();

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(row_count, 1);
}

#[test]
fn test_add_profile_to_collection_missing_profile_errors() {
    let store = MetadataStore::open_in_memory().unwrap();
    let collection_id = store.create_collection("Ghosts").unwrap();

    let result = store.add_profile_to_collection(&collection_id, "does-not-exist");

    match result {
        Err(MetadataStoreError::Validation(msg)) => {
            assert!(
                msg.contains("does-not-exist"),
                "error message should include the missing profile name, got: {msg}"
            );
        }
        other => panic!("expected Validation error, got {other:?}"),
    }

    // Verify no row was inserted.
    let conn = connection(&store);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_rename_collection_updates_name() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Old Name").unwrap();

    store.rename_collection(&id, "New Name").unwrap();

    let collections = store.list_collections().unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "New Name");
}

#[test]
fn test_rename_collection_unknown_id_errors() {
    let store = MetadataStore::open_in_memory().unwrap();
    let result = store.rename_collection("nope", "Whatever");
    assert!(matches!(result, Err(MetadataStoreError::Validation(_))));
}

#[test]
fn test_rename_collection_duplicate_name_errors() {
    let store = MetadataStore::open_in_memory().unwrap();
    let _ = store.create_collection("A").unwrap();
    let id_b = store.create_collection("B").unwrap();

    // Duplicate name violates the UNIQUE constraint on collections.name.
    let result = store.rename_collection(&id_b, "A");
    assert!(
        matches!(result, Err(MetadataStoreError::Database { .. })),
        "duplicate name should bubble as a Database error (UNIQUE violation)"
    );
}

#[test]
fn test_update_collection_description_set_and_clear() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Target").unwrap();

    store
        .update_collection_description(&id, Some("a helpful description"))
        .unwrap();
    let row = store
        .list_collections()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(row.description.as_deref(), Some("a helpful description"));

    // Clearing with Some("   ") normalizes to None.
    store
        .update_collection_description(&id, Some("   "))
        .unwrap();
    let row = store
        .list_collections()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(row.description, None);

    // Clearing with None also works.
    store
        .update_collection_description(&id, Some("again"))
        .unwrap();
    store.update_collection_description(&id, None).unwrap();
    let row = store
        .list_collections()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(row.description, None);
}

#[test]
fn test_collections_for_profile_returns_multi_membership() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");
    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let id_a = store.create_collection("Action").unwrap();
    let id_b = store.create_collection("Backlog").unwrap();
    let _id_c = store.create_collection("Untouched").unwrap();

    store
        .add_profile_to_collection(&id_a, "elden-ring")
        .unwrap();
    store
        .add_profile_to_collection(&id_b, "elden-ring")
        .unwrap();

    let result = store.collections_for_profile("elden-ring").unwrap();
    assert_eq!(result.len(), 2);
    let names: Vec<&str> = result.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Action"));
    assert!(names.contains(&"Backlog"));
    assert!(!names.contains(&"Untouched"));

    // Unknown profile name returns empty vec (not error).
    let empty = store.collections_for_profile("nobody").unwrap();
    assert!(empty.is_empty());
}

#[test]
fn test_profile_delete_cascades_collection_membership() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/vanishing.toml");
    store
        .observe_profile_write("vanishing", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let collection_id = store.create_collection("Ephemeral").unwrap();
    store
        .add_profile_to_collection(&collection_id, "vanishing")
        .unwrap();

    // Hard-delete the profile row (bypassing the soft-delete code path, which
    // only sets deleted_at). We simulate a hard delete to verify the FK cascade.
    let conn = connection(&store);
    conn.execute(
        "DELETE FROM profiles WHERE current_filename = 'vanishing'",
        [],
    )
    .unwrap();
    drop(conn);

    // Membership row must be gone.
    let conn = connection(&store);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "collection_profiles row must cascade on profile delete"
    );
}

#[test]
fn test_collection_delete_cascades() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let collection_id = store.create_collection("To Delete").unwrap();
    store
        .add_profile_to_collection(&collection_id, "elden-ring")
        .unwrap();

    store.delete_collection(&collection_id).unwrap();

    let conn = connection(&store);
    let member_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        member_count, 0,
        "collection_profiles rows should cascade-delete with the collection"
    );
}
