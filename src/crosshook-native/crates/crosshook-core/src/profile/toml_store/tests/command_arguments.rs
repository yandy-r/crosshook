use std::fs;

use tempfile::tempdir;

use crate::launch::request::{ValidationError, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
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
            None,
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
            None,
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

    let result = store.save_command_arguments(
        "missing-profile",
        vec!["force_vulkan".to_string()],
        vec![],
        None,
    );

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
        None,
    );

    assert!(matches!(
        result,
        Err(ProfileStoreError::InvalidCommandArgumentId(id))
            if id == "not_a_real_command_argument"
    ));
}

#[test]
fn save_command_arguments_ignores_blank_custom_tokens() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    store
        .save_command_arguments(
            "elden-ring",
            vec!["force_vulkan".to_string()],
            vec!["   ".to_string()],
            None,
        )
        .expect("blank custom rows should be dropped before validation");

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(
        loaded.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
    assert!(loaded.launch.command_arguments.custom_args.is_empty());
}

#[test]
fn save_command_arguments_rejects_invalid_custom_tokens() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    let result =
        store.save_command_arguments("elden-ring", vec![], vec!["bad\x07arg".to_string()], None);

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
            None,
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

#[test]
fn save_command_arguments_survives_load_modify_full_save_roundtrip() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    store.save("elden-ring", &profile).unwrap();

    store
        .save_command_arguments(
            "elden-ring",
            vec!["skip_launcher".to_string()],
            vec![],
            None,
        )
        .unwrap();

    let mut loaded = store.load("elden-ring").unwrap();
    loaded
        .launch
        .custom_env_vars
        .insert("FOO".to_string(), "BAR".to_string());
    store.save("elden-ring", &loaded).unwrap();

    let reloaded = store.load("elden-ring").unwrap();
    assert_eq!(
        reloaded.launch.command_arguments.enabled_argument_ids,
        vec!["skip_launcher".to_string()]
    );
    assert_eq!(
        reloaded.launch.custom_env_vars.get("FOO"),
        Some(&"BAR".to_string())
    );
}

#[test]
fn save_command_arguments_uses_resolved_launch_method_when_method_field_empty() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();
    profile.launch.method.clear();

    store.save("elden-ring", &profile).unwrap();

    store
        .save_command_arguments("elden-ring", vec!["force_vulkan".to_string()], vec![], None)
        .expect("steam-enabled profile with empty launch.method should resolve to steam_applaunch");

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(
        loaded.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
}

#[test]
fn save_command_arguments_uses_resolved_launch_method_override_when_disk_profile_is_native() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();
    profile.steam.enabled = false;

    store.save("elden-ring", &profile).unwrap();

    store
        .save_command_arguments(
            "elden-ring",
            vec!["skip_launcher".to_string()],
            vec![],
            Some(METHOD_PROTON_RUN),
        )
        .expect("UI-resolved proton_run should validate even when on-disk method is native");

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(loaded.launch.method, "native");
    assert_eq!(
        loaded.launch.command_arguments.enabled_argument_ids,
        vec!["skip_launcher".to_string()]
    );
}

/// Mirrors the exact Steam IPC path: the UI resolves a steam-enabled profile to
/// `steam_applaunch` and passes it as `resolved_launch_method`. This is the
/// scenario the "Failed to save" report covered; it must validate and persist.
#[test]
fn save_command_arguments_accepts_steam_applaunch_override_and_persists() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    assert!(profile.steam.enabled, "fixture should be a Steam profile");

    store.save("elden-ring", &profile).unwrap();

    store
        .save_command_arguments(
            "elden-ring",
            vec!["force_vulkan".to_string()],
            vec![],
            Some(METHOD_STEAM_APPLAUNCH),
        )
        .expect("UI-resolved steam_applaunch should validate for a Steam profile");

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(
        loaded.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
}
