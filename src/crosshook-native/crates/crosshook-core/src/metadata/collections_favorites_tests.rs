#![cfg(test)]

use rusqlite::params;

use super::test_support::{connection, sample_profile};
use super::{MetadataStore, SyncSource};

#[test]
fn test_set_profile_favorite_toggles() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    store.set_profile_favorite("elden-ring", true).unwrap();

    let conn = connection(&store);
    let is_favorite: i64 = conn
        .query_row(
            "SELECT is_favorite FROM profiles WHERE current_filename = ?1",
            params!["elden-ring"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(is_favorite, 1);

    drop(conn);
    store.set_profile_favorite("elden-ring", false).unwrap();

    let conn = connection(&store);
    let is_favorite: i64 = conn
        .query_row(
            "SELECT is_favorite FROM profiles WHERE current_filename = ?1",
            params!["elden-ring"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(is_favorite, 0);
}

#[test]
fn test_list_favorite_profiles_excludes_deleted() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();

    store
        .observe_profile_write(
            "keep-me",
            &profile,
            std::path::Path::new("/profiles/keep-me.toml"),
            SyncSource::AppWrite,
            None,
        )
        .unwrap();
    store
        .observe_profile_write(
            "delete-me",
            &profile,
            std::path::Path::new("/profiles/delete-me.toml"),
            SyncSource::AppWrite,
            None,
        )
        .unwrap();

    store.set_profile_favorite("keep-me", true).unwrap();
    store.set_profile_favorite("delete-me", true).unwrap();
    store.observe_profile_delete("delete-me").unwrap();

    let favorites = store.list_favorite_profiles().unwrap();
    assert_eq!(favorites, vec!["keep-me".to_string()]);
}
