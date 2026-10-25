use std::fs;

use tempfile::tempdir;

use crate::launch::request::{ValidationError, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::profile::toml_store::{ProfileStore, ProfileStoreError};

use super::fixtures::sample_profile;

/// Loads `name`, applies [`GameProfile::apply_command_arguments`], and saves once — the
/// writer's mutate-then-persist sequence.
///
/// [`GameProfile::apply_command_arguments`]: crate::profile::GameProfile::apply_command_arguments
fn apply_and_save(
    store: &ProfileStore,
    name: &str,
    enabled_argument_ids: Vec<String>,
    custom_args: Vec<String>,
    resolved_launch_method: Option<&str>,
) -> Result<(), ProfileStoreError> {
    let mut profile = store.load(name)?;
    profile.apply_command_arguments(enabled_argument_ids, custom_args, resolved_launch_method)?;
    store.save(name, &profile)
}

#[test]
fn apply_command_arguments_persists_only_launch_section() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();

    let enabled_argument_ids = vec!["force_vulkan".to_string()];
    let custom_args = vec!["-windowed".to_string()];
    apply_and_save(
        &store,
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
fn apply_command_arguments_trims_ids_and_custom_tokens() {
    let mut profile = sample_profile();

    profile
        .apply_command_arguments(
            vec!["  force_vulkan  ".to_string(), "   ".to_string()],
            vec!["  -windowed  ".to_string()],
            None,
        )
        .unwrap();

    assert_eq!(
        profile.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
    assert_eq!(
        profile.launch.command_arguments.custom_args,
        vec!["-windowed".to_string()]
    );
}

#[test]
fn apply_command_arguments_rejects_unknown_argument_ids_without_mutating() {
    let mut profile = sample_profile();
    let before = profile.clone();

    let result = profile.apply_command_arguments(
        vec!["not_a_real_command_argument".to_string()],
        vec![],
        None,
    );

    assert!(matches!(
        result,
        Err(ProfileStoreError::InvalidCommandArgumentId(id))
            if id == "not_a_real_command_argument"
    ));
    assert_eq!(profile, before);
}

#[test]
fn apply_command_arguments_rejects_duplicate_argument_ids() {
    let mut profile = sample_profile();

    let result = profile.apply_command_arguments(
        vec!["force_vulkan".to_string(), " force_vulkan ".to_string()],
        vec![],
        None,
    );

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::DuplicateCommandArgument(id)
        )) if id == "force_vulkan"
    ));
}

#[test]
fn apply_command_arguments_ignores_blank_custom_tokens() {
    let mut profile = sample_profile();

    profile
        .apply_command_arguments(
            vec!["force_vulkan".to_string()],
            vec!["   ".to_string()],
            None,
        )
        .expect("blank custom rows should be dropped before validation");

    assert_eq!(
        profile.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
    assert!(profile.launch.command_arguments.custom_args.is_empty());
}

#[test]
fn apply_command_arguments_rejects_invalid_custom_tokens() {
    let mut profile = sample_profile();

    let result = profile.apply_command_arguments(vec![], vec!["bad\x07arg".to_string()], None);

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentCustomTokenContainsControlCharacter
        ))
    ));
}

#[test]
fn apply_command_arguments_writes_only_command_arguments_subsection() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    store.save("elden-ring", &profile).unwrap();

    apply_and_save(
        &store,
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
fn apply_command_arguments_survives_load_modify_full_save_roundtrip() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    store.save("elden-ring", &profile).unwrap();

    apply_and_save(
        &store,
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
fn apply_command_arguments_uses_resolved_launch_method_when_method_field_empty() {
    let mut profile = sample_profile();
    profile.launch.method.clear();

    profile
        .apply_command_arguments(vec!["force_vulkan".to_string()], vec![], None)
        .expect("steam-enabled profile with empty launch.method should resolve to steam_applaunch");

    assert_eq!(
        profile.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan".to_string()]
    );
}

#[test]
fn apply_command_arguments_uses_resolved_launch_method_override_when_profile_is_native() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();
    profile.steam.enabled = false;

    store.save("elden-ring", &profile).unwrap();

    apply_and_save(
        &store,
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
fn apply_command_arguments_accepts_steam_applaunch_override_and_persists() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    assert!(profile.steam.enabled, "fixture should be a Steam profile");

    store.save("elden-ring", &profile).unwrap();

    apply_and_save(
        &store,
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

#[test]
fn apply_command_arguments_rejects_native_without_mutating() {
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();
    profile.steam.enabled = false;
    let before = profile.clone();

    let result = profile.apply_command_arguments(vec![], vec!["-x".into()], None);

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentsUnsupportedForMethod(method)
        )) if method == "native"
    ));
    assert_eq!(profile, before);
}

#[test]
fn apply_command_arguments_rejects_more_than_64_tokens() {
    let mut profile = sample_profile();
    let custom: Vec<String> = (0..65).map(|i| format!("-a{i}")).collect();

    let result = profile.apply_command_arguments(vec![], custom, Some(METHOD_PROTON_RUN));

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentTokenCountExceeded { max_count: 64 }
        ))
    ));
}

#[test]
fn apply_command_arguments_rejects_tokens_longer_than_512_bytes() {
    let mut profile = sample_profile();
    let before = profile.clone();

    let at_limit = profile.clone().apply_command_arguments(
        vec![],
        vec!["a".repeat(512)],
        Some(METHOD_PROTON_RUN),
    );
    assert!(at_limit.is_ok(), "512-byte token is within the limit");

    let result =
        profile.apply_command_arguments(vec![], vec!["a".repeat(513)], Some(METHOD_PROTON_RUN));

    assert!(matches!(
        result,
        Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentTokenTooLong { max_len: 512 }
        ))
    ));
    assert_eq!(profile, before);
}
