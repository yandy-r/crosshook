use std::fs;

use tempfile::tempdir;

use crate::launch::request::ValidationError;
use crate::profile::toml_store::{ProfileStore, ProfileStoreError};

use super::fixtures::sample_profile;

#[test]
fn save_command_arguments_merges_only_launch_section() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();

    let enabled_argument_ids = vec!["force_vulkan".to_string()];
    let custom_args = vec!["-windowed".to_string()];
    store
        .save_command_arguments(
            "elden-ring",
            enabled_argument_ids.clone(),
            custom_args.clone(),
        )
        .unwrap();

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(loaded.game, profile.game);
    assert_eq!(loaded.trainer, profile.trainer);
    assert_eq!(loaded.injection, profile.injection);
    assert_eq!(loaded.steam, profile.steam);
    assert_eq!(loaded.runtime, profile.runtime);
    assert_eq!(loaded.launch.method, profile.launch.method);
    assert_eq!(loaded.launch.optimizations, profile.launch.optimizations);
    assert_eq!(
        loaded.launch.command_arguments.enabled_argument_ids,
        enabled_argument_ids
    );
    assert_eq!(loaded.launch.command_arguments.custom_args, custom_args);
}

#[test]
fn save_command_arguments_trims_ids_and_custom_tokens() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    store
        .save_command_arguments(
            "elden-ring",
            vec!["  force_vulkan  ".to_string()],
            vec!["  -windowed  ".to_string()],
        )
        .unwrap();

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(
        loaded.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
    assert_eq!(
        loaded.launch.command_arguments.custom_args,
        vec!["-windowed".to_string()]
    );
}

#[test]
fn save_command_arguments_rejects_missing_profiles() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

    let result =
        store.save_command_arguments("missing-profile", vec!["force_vulkan".to_string()], vec![]);

    assert!(matches!(result, Err(ProfileStoreError::NotFound(_))));
}

#[test]
fn save_command_arguments_rejects_unknown_argument_ids() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    let result = store.save_command_arguments(
        "elden-ring",
        vec!["not_a_real_command_argument".to_string()],
        vec![],
    );

    assert!(matches!(
        result,
        Err(ProfileStoreError::InvalidCommandArgumentId(id))
            if id == "not_a_real_command_argument"
    ));
}

#[test]
fn save_command_arguments_rejects_blank_custom_tokens() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    let result = store.save_command_arguments("elden-ring", vec![], vec!["   ".to_string()]);

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentCustomTokenEmpty
        ))
    ));
}

#[test]
fn save_command_arguments_rejects_invalid_custom_tokens() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    let result = store.save_command_arguments("elden-ring", vec![], vec!["bad\x07arg".to_string()]);

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentCustomTokenContainsControlCharacter
        ))
    ));
}

#[test]
fn save_command_arguments_writes_only_command_arguments_subsection() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    store.save("elden-ring", &profile).unwrap();

    store
        .save_command_arguments(
            "elden-ring",
            vec!["skip_launcher".to_string()],
            vec!["+set sv_cheats 1".to_string()],
        )
        .unwrap();

    let raw = fs::read_to_string(store.profile_path("elden-ring").unwrap()).unwrap();
    assert!(raw.contains("[launch.command_arguments]"));
    assert!(raw.contains("enabled_argument_ids = [\"skip_launcher\"]"));
    assert!(raw.contains("custom_args = [\"+set sv_cheats 1\"]"));
    assert!(!raw.contains("[launch.command_arguments.command_arguments]"));

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(loaded.launch.optimizations, profile.launch.optimizations);
}
