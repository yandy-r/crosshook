use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;

use super::*;
use crate::profile::models::GameProfile;
use crate::profile::toml_store::ProfileStore;
use crate::steam::proton::normalize_alias;
use crate::steam::ProtonInstall;

// --- extract_proton_family ---

#[test]
fn family_ge_proton_modern() {
    assert_eq!(
        extract_proton_family("GE-Proton9-7"),
        Some("geproton".to_string())
    );
}

#[test]
fn family_ge_proton_double_digit_major() {
    assert_eq!(
        extract_proton_family("GE-Proton10-34"),
        Some("geproton".to_string())
    );
}

#[test]
fn family_official_proton() {
    assert_eq!(
        extract_proton_family("Proton 9.0"),
        Some("proton".to_string())
    );
}

#[test]
fn family_proton_experimental() {
    assert_eq!(
        extract_proton_family("Proton Experimental"),
        Some("protonexperimental".to_string())
    );
}

#[test]
fn family_tkg_returns_fixed_key() {
    assert_eq!(
        extract_proton_family("proton_tkg_6.17.r0.g5f19a815.release"),
        Some("protontkg".to_string())
    );
}

#[test]
fn family_legacy_ge() {
    // "Proton-9.23-GE-2" normalizes to "proton923ge2", trailing digit stripped → "proton923ge".
    // In Phase 1 this is a separate family from modern GE ("geproton") — by design.
    assert_eq!(
        extract_proton_family("Proton-9.23-GE-2"),
        Some("proton923ge".to_string())
    );
}

// --- extract_version_segments ---

#[test]
fn version_ge_proton_double_digit_major() {
    assert_eq!(extract_version_segments("GE-Proton10-34"), vec![10u32, 34]);
}

#[test]
fn version_official_proton() {
    assert_eq!(extract_version_segments("Proton 9.0-1"), vec![9u32, 0, 1]);
}

#[test]
fn version_experimental_is_empty() {
    assert!(extract_version_segments("Proton Experimental").is_empty());
}

#[test]
fn version_ge_proton_single_digit() {
    assert_eq!(extract_version_segments("GE-Proton9-7"), vec![9u32, 7]);
}

// --- integer-tuple ordering (critical correctness test) ---

#[test]
fn version_tuple_ordering_multi_digit_build() {
    // [9, 10] must sort AFTER [9, 9] — lexicographic comparison gets this wrong.
    let v1: Vec<u32> = vec![9, 10];
    let v2: Vec<u32> = vec![9, 9];
    assert!(v1 > v2, "[9, 10] must be greater than [9, 9]");
}

#[test]
fn version_tuple_ordering_cross_major() {
    let v10_1: Vec<u32> = vec![10, 1];
    let v9_99: Vec<u32> = vec![9, 99];
    assert!(v10_1 > v9_99, "[10, 1] must be greater than [9, 99]");
}

// --- find_best_replacement ---

fn make_install(name: &str, path: &str, is_official: bool) -> ProtonInstall {
    let mut normalized_aliases = BTreeSet::new();
    if let Some(n) = normalize_alias(name) {
        normalized_aliases.insert(n);
    }
    ProtonInstall {
        name: name.to_string(),
        path: PathBuf::from(path),
        is_official,
        aliases: vec![name.to_string()],
        normalized_aliases,
    }
}

#[test]
fn same_family_newer_gets_09_confidence() {
    let installed = vec![make_install(
        "GE-Proton9-7",
        "/compat/GE-Proton9-7/proton",
        false,
    )];
    let result = find_best_replacement("GE-Proton9-4", &installed);
    let (install, confidence, crosses_major) = result.expect("should find replacement");
    assert_eq!(install.name, "GE-Proton9-7");
    assert!((confidence - 0.9_f64).abs() < f64::EPSILON);
    assert!(!crosses_major);
}

#[test]
fn cross_major_gets_075_confidence_and_crosses_major_true() {
    let installed = vec![make_install(
        "GE-Proton10-1",
        "/compat/GE-Proton10-1/proton",
        false,
    )];
    let result = find_best_replacement("GE-Proton9-7", &installed);
    let (install, confidence, crosses_major) = result.expect("should find replacement");
    assert_eq!(install.name, "GE-Proton10-1");
    assert!((confidence - 0.75_f64).abs() < f64::EPSILON);
    assert!(crosses_major);
}

#[test]
fn same_family_older_gets_07_confidence() {
    let installed = vec![make_install(
        "GE-Proton9-3",
        "/compat/GE-Proton9-3/proton",
        false,
    )];
    let result = find_best_replacement("GE-Proton9-7", &installed);
    let (install, confidence, crosses_major) = result.expect("should find older replacement");
    assert_eq!(install.name, "GE-Proton9-3");
    assert!((confidence - 0.7_f64).abs() < f64::EPSILON);
    assert!(!crosses_major);
}

#[test]
fn picks_newest_when_multiple_same_family_candidates() {
    let installed = vec![
        make_install("GE-Proton9-5", "/compat/GE-Proton9-5/proton", false),
        make_install("GE-Proton9-10", "/compat/GE-Proton9-10/proton", false),
        make_install("GE-Proton9-7", "/compat/GE-Proton9-7/proton", false),
    ];
    let result = find_best_replacement("GE-Proton9-4", &installed);
    let (install, _confidence, _) = result.expect("should find replacement");
    // [9, 10] > [9, 7] > [9, 5]
    assert_eq!(install.name, "GE-Proton9-10");
}

#[test]
fn proton_experimental_only_matches_another_experimental() {
    let installed = vec![
        make_install("GE-Proton9-7", "/compat/GE-Proton9-7/proton", false),
        make_install(
            "Proton Experimental",
            "/steam/Proton Experimental/proton",
            true,
        ),
    ];
    let result = find_best_replacement("Proton Experimental", &installed);
    let (install, confidence, crosses_major) = result.expect("should find experimental");
    assert_eq!(install.name, "Proton Experimental");
    assert!((confidence - 0.8_f64).abs() < f64::EPSILON);
    assert!(!crosses_major);
}

#[test]
fn proton_experimental_no_match_when_none_installed() {
    let installed = vec![make_install(
        "GE-Proton9-7",
        "/compat/GE-Proton9-7/proton",
        false,
    )];
    assert!(
        find_best_replacement("Proton Experimental", &installed).is_none(),
        "Experimental must not match GE-Proton"
    );
}

#[test]
fn no_match_returns_none() {
    let installed = vec![make_install(
        "Proton 9.0",
        "/steam/common/Proton 9.0/proton",
        true,
    )];
    // Stale is GE-Proton, installed is official Proton — different family.
    assert!(find_best_replacement("GE-Proton9-4", &installed).is_none());
}

#[test]
fn tkg_returns_none() {
    let installed = vec![
        make_install(
            "proton_tkg_6.17.r0.g5f19a815.release",
            "/compat/tkg/proton",
            false,
        ),
        make_install("GE-Proton9-7", "/compat/GE-Proton9-7/proton", false),
    ];
    // A stale TKG install should never receive an auto-suggestion.
    assert!(find_best_replacement("proton_tkg_6.17.r0.g5f19a815.release", &installed).is_none());
}

// --- apply_single_migration round-trip ---

#[test]
fn round_trip_migration_updates_steam_proton_path() {
    let dir = tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(dir.path().to_path_buf());

    // Create a profile with a stale steam.proton_path.
    let mut profile = GameProfile::default();
    profile.launch.method = "steam_applaunch".to_string();
    profile.steam.proton_path = "/stale/GE-Proton9-4/proton".to_string();
    store.save("test-game", &profile).expect("initial save");

    // Create the replacement proton executable.
    let new_proton_dir = dir.path().join("GE-Proton9-7");
    fs::create_dir_all(&new_proton_dir).expect("mkdir new proton dir");
    let new_proton_path = new_proton_dir.join("proton");
    fs::write(&new_proton_path, b"#!/bin/sh\n").expect("write proton file");

    let new_path_str = new_proton_path.to_string_lossy().into_owned();

    // Apply the migration.
    let request = ApplyMigrationRequest {
        profile_name: "test-game".to_string(),
        field: ProtonPathField::SteamProtonPath,
        new_path: new_path_str.clone(),
    };
    let result = apply_single_migration(&store, &request);

    assert_eq!(
        result.outcome,
        MigrationOutcome::Applied,
        "migration should apply successfully; error: {:?}",
        result.error
    );
    assert!(result.error.is_none());
    assert_eq!(result.new_path, new_path_str);

    // Re-load and verify effective path is updated.
    let reloaded = store.load("test-game").expect("reload");
    assert_eq!(
        reloaded.steam.proton_path, new_path_str,
        "effective path must reflect the migration"
    );

    // Verify the on-disk TOML stores the path in local_override.
    let toml_content = fs::read_to_string(dir.path().join("test-game.toml")).expect("read toml");
    assert!(
        toml_content.contains(&new_path_str),
        "new path must appear in TOML"
    );
    assert!(
        toml_content.contains("[local_override"),
        "path must be under local_override in TOML"
    );
}

#[test]
fn migration_already_valid_when_old_path_exists() {
    let dir = tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(dir.path().to_path_buf());

    // Create a "valid" proton executable that actually exists.
    let proton_dir = dir.path().join("GE-Proton9-4");
    fs::create_dir_all(&proton_dir).expect("mkdir");
    let proton_path = proton_dir.join("proton");
    fs::write(&proton_path, b"#!/bin/sh\n").expect("write proton");

    let mut profile = GameProfile::default();
    profile.launch.method = "steam_applaunch".to_string();
    profile.steam.proton_path = proton_path.to_string_lossy().into_owned();
    store.save("valid-game", &profile).expect("save");

    // Create the "new" replacement.
    let new_proton_dir = dir.path().join("GE-Proton9-7");
    fs::create_dir_all(&new_proton_dir).expect("mkdir");
    let new_proton_path = new_proton_dir.join("proton");
    fs::write(&new_proton_path, b"#!/bin/sh\n").expect("write new proton");

    let request = ApplyMigrationRequest {
        profile_name: "valid-game".to_string(),
        field: ProtonPathField::SteamProtonPath,
        new_path: new_proton_path.to_string_lossy().into_owned(),
    };
    let result = apply_single_migration(&store, &request);

    assert_eq!(result.outcome, MigrationOutcome::AlreadyValid);
}

#[test]
fn migration_fails_when_replacement_does_not_exist() {
    let dir = tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(dir.path().to_path_buf());

    let mut profile = GameProfile::default();
    profile.launch.method = "steam_applaunch".to_string();
    profile.steam.proton_path = "/stale/proton".to_string();
    store.save("broken-game", &profile).expect("save");

    let request = ApplyMigrationRequest {
        profile_name: "broken-game".to_string(),
        field: ProtonPathField::SteamProtonPath,
        new_path: "/nonexistent/proton".to_string(),
    };
    let result = apply_single_migration(&store, &request);

    assert_eq!(result.outcome, MigrationOutcome::Failed);
    assert!(result.error.is_some());
}
