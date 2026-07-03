#![cfg(test)]
//! Launch-collector behavior over a real (in-memory) metadata store:
//! degraded-store fallbacks, path-missing checks, and panel/preview parity.

use std::collections::BTreeSet;
use std::fs;

use tempfile::tempdir;

use super::test_support::{connection, insert_test_profile_row, sample_profile};
use super::MetadataStore;
use crate::launch::mod_coexistence::{
    analyze_profile_mod_coexistence, collect_mod_coexistence_launch_warnings,
};
use crate::launch::request::LaunchRequest;
use crate::mods::{ModCategory, ModProvenance, ProfileModInput};
use crate::profile::TrainerLoadingMode;

const PROFILE_ID: &str = "profile-1";

fn store_with_profile() -> MetadataStore {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, PROFILE_ID);
    }
    store
}

fn add_mod(store: &MetadataStore, name: &str, category: ModCategory, paths: &[&str]) {
    store
        .add_profile_mod(
            PROFILE_ID,
            &ProfileModInput {
                name: name.to_string(),
                category,
                paths: paths.iter().map(ToString::to_string).collect(),
                enabled: true,
                source_url: None,
                provenance: ModProvenance::Manual,
            },
        )
        .unwrap();
}

fn codes(issues: &[crate::launch::request::LaunchValidationIssue]) -> BTreeSet<String> {
    issues
        .iter()
        .filter_map(|issue| issue.code.clone())
        .collect()
}

#[test]
fn collector_returns_empty_when_store_disabled() {
    let store = MetadataStore::disabled();
    let profile = sample_profile();
    let request = LaunchRequest::default();

    let issues = collect_mod_coexistence_launch_warnings(&store, PROFILE_ID, &profile, &request);
    assert!(
        issues.is_empty(),
        "a disabled store must degrade, never block"
    );
}

#[test]
fn collector_returns_empty_for_unknown_profile() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let request = LaunchRequest::default();

    let issues =
        collect_mod_coexistence_launch_warnings(&store, "no-such-profile", &profile, &request);
    assert!(issues.is_empty());
}

#[test]
fn collector_returns_empty_on_corrupt_paths_json() {
    let store = store_with_profile();
    add_mod(&store, "Corrupted", ModCategory::FileReplacement, &[]);
    {
        let conn = connection(&store);
        conn.execute("UPDATE profile_mods SET paths_json = '{'", [])
            .unwrap();
    }

    let issues = collect_mod_coexistence_launch_warnings(
        &store,
        PROFILE_ID,
        &sample_profile(),
        &LaunchRequest::default(),
    );
    assert!(
        issues.is_empty(),
        "store errors must degrade, never block launch"
    );
}

#[test]
fn collector_maps_codes_onto_issues() {
    let store = store_with_profile();
    add_mod(&store, "SKSE64", ModCategory::ScriptExtender, &[]);
    add_mod(&store, "HD Texture Pack", ModCategory::FileReplacement, &[]);

    let profile = sample_profile();
    let request = LaunchRequest {
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        ..LaunchRequest::default()
    };

    let issues = collect_mod_coexistence_launch_warnings(&store, PROFILE_ID, &profile, &request);
    assert_eq!(
        codes(&issues),
        BTreeSet::from([
            "mod_coexistence_injection_vector".to_string(),
            "mod_coexistence_file_replacement_notice".to_string(),
            "mod_coexistence_script_extender_launch_order".to_string(),
        ])
    );
}

#[test]
fn panel_registered_path_missing_fires_only_when_game_dir_known_and_path_absent() {
    let store = store_with_profile();
    add_mod(
        &store,
        "Missing Files",
        ModCategory::Other,
        &["missing.dll"],
    );
    add_mod(
        &store,
        "Present Files",
        ModCategory::Other,
        &["present.dll"],
    );

    let game_dir = tempdir().unwrap();
    fs::write(game_dir.path().join("present.dll"), b"").unwrap();

    let mut profile = sample_profile();
    profile.trainer.path = String::new();
    profile.game.executable_path = game_dir
        .path()
        .join("game.exe")
        .to_string_lossy()
        .into_owned();

    let issues = analyze_profile_mod_coexistence(&store, PROFILE_ID, &profile).unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].code.as_deref(),
        Some("mod_coexistence_registered_path_missing")
    );
    assert!(issues[0].message.contains("Missing Files"));
    assert!(issues[0].message.contains("missing.dll"));

    // Unknown game dir: relative paths are skipped entirely.
    profile.game.executable_path = String::new();
    let issues = analyze_profile_mod_coexistence(&store, PROFILE_ID, &profile).unwrap();
    assert!(
        issues.is_empty(),
        "relative paths must be skipped when the game dir is unknown: {issues:?}"
    );
}

#[test]
fn panel_registered_path_missing_checks_absolute_paths_without_game_dir() {
    let store = store_with_profile();
    add_mod(
        &store,
        "Absolute Mod",
        ModCategory::Other,
        &["/nonexistent/crosshook-mods-test/missing.dll"],
    );

    let mut profile = sample_profile();
    profile.trainer.path = String::new();
    profile.game.executable_path = String::new();

    let issues = analyze_profile_mod_coexistence(&store, PROFILE_ID, &profile).unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].code.as_deref(),
        Some("mod_coexistence_registered_path_missing")
    );
}

#[test]
fn preview_collector_never_emits_registered_path_missing() {
    let store = store_with_profile();
    add_mod(
        &store,
        "Missing Files",
        ModCategory::Other,
        &[
            "missing.dll",
            "/nonexistent/crosshook-mods-test/missing.dll",
        ],
    );

    let game_dir = tempdir().unwrap();
    let mut profile = sample_profile();
    profile.trainer.path = String::new();
    profile.game.executable_path = game_dir
        .path()
        .join("game.exe")
        .to_string_lossy()
        .into_owned();
    let request = LaunchRequest {
        game_path: profile.game.executable_path.clone(),
        ..LaunchRequest::default()
    };

    let issues = collect_mod_coexistence_launch_warnings(&store, PROFILE_ID, &profile, &request);
    assert!(
        !codes(&issues).contains("mod_coexistence_registered_path_missing"),
        "the per-preview/launch collector must never stat registered paths: {issues:?}"
    );
}

#[test]
fn profile_variant_and_request_variant_agree_on_same_profile() {
    let store = store_with_profile();
    add_mod(&store, "SKSE64", ModCategory::ScriptExtender, &[]);
    add_mod(
        &store,
        "ReShade",
        ModCategory::OverlayInjection,
        &["dxgi.dll"],
    );

    let profile = sample_profile();
    let request = LaunchRequest {
        method: profile.launch.method.clone(),
        game_path: profile.game.executable_path.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        ..LaunchRequest::default()
    };

    let via_request =
        collect_mod_coexistence_launch_warnings(&store, PROFILE_ID, &profile, &request);
    // The panel additionally runs the filesystem path-missing pass; rule
    // advisories themselves must not drift between the two entry points.
    let via_profile: Vec<_> = analyze_profile_mod_coexistence(&store, PROFILE_ID, &profile)
        .unwrap()
        .into_iter()
        .filter(|issue| issue.code.as_deref() != Some("mod_coexistence_registered_path_missing"))
        .collect();

    assert!(!via_request.is_empty(), "fixture should produce advisories");
    assert_eq!(
        via_request, via_profile,
        "editor panel and launch preview must not drift"
    );
}
