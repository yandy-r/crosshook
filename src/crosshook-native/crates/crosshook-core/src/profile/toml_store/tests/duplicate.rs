use tempfile::tempdir;

use crate::profile::toml_store::{ProfileStore, ProfileStoreError};

use super::fixtures::sample_profile;

#[test]
fn test_strip_copy_suffix() {
    use super::super::utils::strip_copy_suffix;

    assert_eq!(strip_copy_suffix("Name (Copy)"), "Name");
    assert_eq!(strip_copy_suffix("Name (Copy 3)"), "Name");
    assert_eq!(strip_copy_suffix("Name"), "Name");
    assert_eq!(strip_copy_suffix("Copy"), "Copy");
    assert_eq!(
        strip_copy_suffix("Game (Special Edition)"),
        "Game (Special Edition)"
    );
    assert_eq!(strip_copy_suffix("Name (Copy 0)"), "Name");
    assert_eq!(strip_copy_suffix("Name (Copy 99)"), "Name");
}

#[test]
fn test_duplicate_basic() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("MyGame", &profile).unwrap();
    let result = store.duplicate("MyGame").unwrap();

    assert_eq!(result.name, "MyGame (Copy)");
    assert_eq!(result.profile, profile);
    assert!(store.profile_path("MyGame (Copy)").unwrap().exists());
}

#[test]
fn test_duplicate_increments_on_conflict() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("MyGame", &profile).unwrap();
    store.save("MyGame (Copy)", &profile).unwrap();
    let result = store.duplicate("MyGame").unwrap();

    assert_eq!(result.name, "MyGame (Copy 2)");
}

#[test]
fn test_duplicate_of_copy() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("MyGame", &profile).unwrap();
    store.save("MyGame (Copy)", &profile).unwrap();
    let result = store.duplicate("MyGame (Copy)").unwrap();

    assert_eq!(result.name, "MyGame (Copy 2)");
}

#[test]
fn test_duplicate_copy_suffix_only_name_keeps_non_empty_base() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("(Copy)", &profile).unwrap();
    let result = store.duplicate("(Copy)").unwrap();

    assert_eq!(result.name, "(Copy) (Copy)");
    assert!(!result.name.starts_with(' '));
    assert_eq!(store.load(&result.name).unwrap(), profile);
}

#[test]
fn test_duplicate_preserves_all_fields() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("FullProfile", &profile).unwrap();
    let result = store.duplicate("FullProfile").unwrap();

    let loaded_source = store.load("FullProfile").unwrap();
    let loaded_copy = store.load(&result.name).unwrap();
    assert_eq!(loaded_source, loaded_copy);
}

#[test]
fn test_duplicate_source_not_found() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    std::fs::create_dir_all(&store.base_path).unwrap();

    let result = store.duplicate("nonexistent");
    assert!(matches!(result, Err(ProfileStoreError::NotFound(_))));
}
