#![cfg(test)]

use chrono::DateTime;

use super::test_support::{connection, insert_test_profile_row};
use super::{MetadataStore, MetadataStoreError};
use crate::mods::{ModCategory, ModProvenance, ProfileModInput};

fn store_with_profile(profile_id: &str) -> MetadataStore {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, profile_id);
    }
    store
}

fn input(name: &str) -> ProfileModInput {
    ProfileModInput {
        name: name.to_string(),
        category: ModCategory::Other,
        paths: Vec::new(),
        enabled: true,
        source_url: None,
        provenance: ModProvenance::Manual,
    }
}

fn expect_validation(result: Result<impl std::fmt::Debug, MetadataStoreError>, message: &str) {
    match result {
        Err(MetadataStoreError::Validation(actual)) => {
            assert_eq!(actual, message);
        }
        other => panic!("expected Validation({message:?}), got {other:?}"),
    }
}

#[test]
fn add_and_list_round_trip() {
    let store = store_with_profile("profile-1");
    let payload = ProfileModInput {
        name: "SKSE64".to_string(),
        category: ModCategory::ScriptExtender,
        paths: vec!["skse64_loader.exe".to_string(), "Data/SKSE".to_string()],
        enabled: true,
        source_url: Some("https://skse.silverlock.org".to_string()),
        provenance: ModProvenance::Detected,
    };

    let added = store.add_profile_mod("profile-1", &payload).unwrap();
    assert!(!added.mod_id.is_empty());
    assert_eq!(added.profile_id, "profile-1");
    assert_eq!(added.name, "SKSE64");
    assert_eq!(added.category, ModCategory::ScriptExtender);
    assert_eq!(
        added.paths,
        vec!["skse64_loader.exe".to_string(), "Data/SKSE".to_string()],
        "path order must be preserved"
    );
    assert!(added.enabled);
    assert_eq!(added.provenance, ModProvenance::Detected);
    assert_eq!(
        added.source_url.as_deref(),
        Some("https://skse.silverlock.org")
    );
    assert!(DateTime::parse_from_rfc3339(&added.created_at).is_ok());
    assert!(DateTime::parse_from_rfc3339(&added.updated_at).is_ok());
    assert_eq!(added.created_at, added.updated_at);

    let listed = store.list_profile_mods("profile-1").unwrap();
    assert_eq!(listed, vec![added]);
}

#[test]
fn list_orders_by_name_case_insensitive() {
    let store = store_with_profile("profile-1");
    for name in ["beta", "GAMMA", "Alpha"] {
        store.add_profile_mod("profile-1", &input(name)).unwrap();
    }

    let names: Vec<String> = store
        .list_profile_mods("profile-1")
        .unwrap()
        .into_iter()
        .map(|record| record.name)
        .collect();
    assert_eq!(names, vec!["Alpha", "beta", "GAMMA"]);
}

#[test]
fn duplicate_name_same_profile_rejected_with_friendly_message() {
    let store = store_with_profile("profile-1");
    store.add_profile_mod("profile-1", &input("SKSE")).unwrap();

    match store.add_profile_mod("profile-1", &input("SKSE")) {
        Err(MetadataStoreError::Validation(message)) => {
            assert_eq!(
                message,
                "a mod named “SKSE” is already registered for this profile"
            );
        }
        other => panic!("expected friendly duplicate-name validation error, got {other:?}"),
    }
}

#[test]
fn duplicate_name_differing_only_by_case_rejected_with_friendly_message() {
    let store = store_with_profile("profile-1");
    store
        .add_profile_mod("profile-1", &input("ReShade"))
        .unwrap();

    match store.add_profile_mod("profile-1", &input("reshade")) {
        Err(MetadataStoreError::Validation(message)) => {
            assert_eq!(
                message,
                "a mod named “reshade” is already registered for this profile"
            );
        }
        other => panic!("expected friendly duplicate-name validation error, got {other:?}"),
    }
}

#[test]
fn same_name_different_profiles_allowed() {
    let store = store_with_profile("profile-1");
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "profile-2");
    }

    store.add_profile_mod("profile-1", &input("SKSE")).unwrap();
    store.add_profile_mod("profile-2", &input("SKSE")).unwrap();

    assert_eq!(store.list_profile_mods("profile-1").unwrap().len(), 1);
    assert_eq!(store.list_profile_mods("profile-2").unwrap().len(), 1);
}

#[test]
fn update_preserves_created_at_and_bumps_updated_at() {
    let store = store_with_profile("profile-1");
    let added = store
        .add_profile_mod("profile-1", &input("ReShade"))
        .unwrap();

    let mut next = input("ReShade Renamed");
    next.category = ModCategory::OverlayInjection;
    next.enabled = false;
    let updated = store
        .update_profile_mod("profile-1", &added.mod_id, &next)
        .unwrap();

    assert_eq!(updated.mod_id, added.mod_id);
    assert_eq!(updated.name, "ReShade Renamed");
    assert_eq!(updated.category, ModCategory::OverlayInjection);
    assert!(!updated.enabled);
    assert_eq!(
        updated.created_at, added.created_at,
        "created_at must be preserved"
    );
    let created = DateTime::parse_from_rfc3339(&updated.created_at).unwrap();
    let bumped = DateTime::parse_from_rfc3339(&updated.updated_at).unwrap();
    assert!(bumped >= created, "updated_at must be bumped on update");
}

#[test]
fn update_missing_mod_is_validation_error() {
    let store = store_with_profile("profile-1");
    expect_validation(
        store.update_profile_mod("profile-1", "no-such-mod", &input("X")),
        "mod not found",
    );
}

#[test]
fn remove_returns_true_then_false() {
    let store = store_with_profile("profile-1");
    let added = store.add_profile_mod("profile-1", &input("X")).unwrap();

    assert!(store
        .remove_profile_mod("profile-1", &added.mod_id)
        .unwrap());
    assert!(!store
        .remove_profile_mod("profile-1", &added.mod_id)
        .unwrap());
    assert!(store.list_profile_mods("profile-1").unwrap().is_empty());
}

#[test]
fn list_enabled_filters_disabled() {
    let store = store_with_profile("profile-1");
    store
        .add_profile_mod("profile-1", &input("Enabled Mod"))
        .unwrap();
    let mut disabled = input("Disabled Mod");
    disabled.enabled = false;
    store.add_profile_mod("profile-1", &disabled).unwrap();

    let enabled = store.list_enabled_profile_mods("profile-1").unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].name, "Enabled Mod");
    assert_eq!(store.list_profile_mods("profile-1").unwrap().len(), 2);
}

#[test]
fn validation_rejects_empty_name() {
    let store = store_with_profile("profile-1");
    expect_validation(
        store.add_profile_mod("profile-1", &input("   ")),
        "mod name is required",
    );
}

#[test]
fn validation_rejects_long_name() {
    let store = store_with_profile("profile-1");
    expect_validation(
        store.add_profile_mod("profile-1", &input(&"x".repeat(201))),
        "mod name exceeds 200 characters",
    );
}

#[test]
fn validation_rejects_too_many_paths() {
    let store = store_with_profile("profile-1");
    let mut payload = input("Many Paths");
    payload.paths = (0..33).map(|index| format!("path-{index}.dll")).collect();
    expect_validation(
        store.add_profile_mod("profile-1", &payload),
        "a mod may register at most 32 paths",
    );
}

#[test]
fn validation_rejects_long_path() {
    let store = store_with_profile("profile-1");
    let mut payload = input("Long Path");
    payload.paths = vec!["p".repeat(1025)];
    expect_validation(
        store.add_profile_mod("profile-1", &payload),
        "mod path exceeds 1024 characters",
    );
}

#[test]
fn validation_rejects_nul_in_path() {
    let store = store_with_profile("profile-1");
    let mut payload = input("NUL Path");
    payload.paths = vec!["bad\0path.dll".to_string()];
    expect_validation(
        store.add_profile_mod("profile-1", &payload),
        "mod path may not contain NUL",
    );
}

#[test]
fn validation_rejects_paths_over_budget() {
    let store = store_with_profile("profile-1");
    let mut payload = input("Budget Buster");
    payload.paths = (0..32)
        .map(|index| format!("{index}-{}", "p".repeat(300)))
        .collect();
    expect_validation(
        store.add_profile_mod("profile-1", &payload),
        "mod paths exceed the 8 KiB storage budget",
    );
}

#[test]
fn validation_rejects_non_http_source_url() {
    let store = store_with_profile("profile-1");
    let mut payload = input("Bad Url");
    payload.source_url = Some("ftp://example.com/mod".to_string());
    expect_validation(
        store.add_profile_mod("profile-1", &payload),
        "source_url must be an http(s) URL",
    );
}

#[test]
fn validation_rejects_long_source_url() {
    let store = store_with_profile("profile-1");
    let mut payload = input("Long Url");
    payload.source_url = Some(format!("https://example.com/{}", "u".repeat(2048)));
    expect_validation(
        store.add_profile_mod("profile-1", &payload),
        "source_url exceeds 2048 characters",
    );
}

#[test]
fn validation_normalizes_trims_and_drops_empty_paths() {
    let store = store_with_profile("profile-1");
    let payload = ProfileModInput {
        name: "  Trimmed Mod  ".to_string(),
        category: ModCategory::Other,
        paths: vec!["  a.dll  ".to_string(), String::new(), "   ".to_string()],
        enabled: true,
        source_url: Some("   ".to_string()),
        provenance: ModProvenance::Manual,
    };

    let added = store.add_profile_mod("profile-1", &payload).unwrap();
    assert_eq!(added.name, "Trimmed Mod");
    assert_eq!(added.paths, vec!["a.dll".to_string()]);
    assert_eq!(added.source_url, None);
}

#[test]
fn corrupt_paths_json_is_typed_corrupt_error() {
    let store = store_with_profile("profile-1");
    store
        .add_profile_mod("profile-1", &input("Corrupted"))
        .unwrap();
    {
        let conn = connection(&store);
        conn.execute("UPDATE profile_mods SET paths_json = '{'", [])
            .unwrap();
    }

    match store.list_profile_mods("profile-1") {
        Err(MetadataStoreError::Corrupt(message)) => {
            assert!(
                message.contains("profile_mods.paths_json unreadable"),
                "unexpected corrupt message: {message}"
            );
        }
        other => panic!("expected Corrupt error for unreadable paths_json, got {other:?}"),
    }
}

#[test]
fn disabled_store_errors_loudly() {
    let store = MetadataStore::disabled();
    assert!(
        store.add_profile_mod("profile-1", &input("X")).is_err(),
        "a disabled store must error, never silently no-op"
    );
    assert!(store.list_profile_mods("profile-1").is_err());
}
