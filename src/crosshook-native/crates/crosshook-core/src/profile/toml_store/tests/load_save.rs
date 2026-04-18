use std::fs;

use tempfile::tempdir;

use super::super::utils::validate_name;
use crate::profile::models::{CollectionDefaultsSection, LocalOverrideSection};
use crate::profile::toml_store::ProfileStore;

use super::fixtures::sample_profile;

#[test]
fn save_load_list_and_delete_round_trip() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();
    assert_eq!(store.list().unwrap(), vec!["elden-ring".to_string()]);
    assert_eq!(store.load("elden-ring").unwrap(), profile);

    store.delete("elden-ring").unwrap();
    assert!(store.load("elden-ring").is_err());
    assert!(store.list().unwrap().is_empty());
}

/// Regression test for M2 of the Phase 3 review (docs/prps/reviews/pr-184-review.md).
///
/// Exercises the real runtime pipeline used by the `profile_load` Tauri command:
///
///   1. Save a profile whose `local_override` section carries machine-specific paths.
///   2. `ProfileStore::load` bakes those overrides into layer 1 and clears
///      `local_override` to `LocalOverrideSection::default()`.
///   3. `effective_profile_with(Some(&defaults))` then merges collection defaults on
///      top of the already-flattened profile.
///
/// Asserts that:
/// - `local_override`-flavoured paths (executable_path, cover art) still win at the
///   final call site even though layer 3 is technically a no-op at that point —
///   because the save-then-load step has already baked them into layer 1.
/// - Collection defaults successfully override launch fields that do not overlap
///   with `local_override` (method, env vars, network isolation).
///
/// This locks in the behaviour documented on `effective_profile_with` so a future
/// contributor extending `local_override` with overlapping fields will see a failing
/// test if they break the "local_override always wins" runtime invariant.
#[test]
fn save_load_then_merge_collection_defaults_preserves_local_override_paths() {
    use std::collections::BTreeMap;

    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

    let mut profile = sample_profile();
    profile.game.executable_path = "/portable/elden-ring.exe".to_string();
    profile.game.custom_cover_art_path = "/portable/cover.png".to_string();
    profile.launch.method = "native".to_string();
    profile.launch.network_isolation = true;
    profile
        .launch
        .custom_env_vars
        .insert("PROFILE_ONLY".to_string(), "keep-me".to_string());
    profile.local_override.game.executable_path = "/local/elden-ring.exe".to_string();
    profile.local_override.game.custom_cover_art_path = "/local/cover.png".to_string();

    store.save("elden-ring", &profile).unwrap();

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(
        loaded.game.executable_path, "/local/elden-ring.exe",
        "local_override executable_path should be baked into layer 1"
    );
    assert_eq!(
        loaded.game.custom_cover_art_path, "/local/cover.png",
        "local_override cover art should be baked into layer 1"
    );
    assert_eq!(
        loaded.local_override,
        LocalOverrideSection::default(),
        "post-load profile must have local_override cleared — that's what makes \
         layer 3 a no-op at the profile_load call site"
    );

    let mut defaults = CollectionDefaultsSection::default();
    defaults.method = Some("proton_run".to_string());
    defaults.network_isolation = Some(false);
    let mut env = BTreeMap::new();
    env.insert("COLLECTION_ONLY".to_string(), "from-collection".to_string());
    env.insert("PROFILE_ONLY".to_string(), "overridden".to_string());
    defaults.custom_env_vars = env;

    let merged = loaded.effective_profile_with(Some(&defaults));

    assert_eq!(
        merged.game.executable_path, "/local/elden-ring.exe",
        "local_override executable_path must survive the merge — this is the \
         runtime 'local_override always wins' guarantee"
    );
    assert_eq!(
        merged.game.custom_cover_art_path, "/local/cover.png",
        "local_override cover art must survive the merge"
    );

    assert_eq!(merged.launch.method, "proton_run");
    assert!(!merged.launch.network_isolation);
    assert_eq!(
        merged
            .launch
            .custom_env_vars
            .get("COLLECTION_ONLY")
            .cloned(),
        Some("from-collection".to_string())
    );
    assert_eq!(
        merged.launch.custom_env_vars.get("PROFILE_ONLY").cloned(),
        Some("overridden".to_string())
    );
}

#[test]
fn load_defaults_runtime_when_runtime_section_is_missing() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile_path = store.base_path.join("legacy.toml");

    fs::create_dir_all(&store.base_path).unwrap();
    fs::write(
        &profile_path,
        r#"[game]
name = "Legacy"
executable_path = "/games/legacy.sh"

[trainer]
path = "/trainers/legacy"
type = "native"

[injection]
dll_paths = []
inject_on_launch = [false, false]

[steam]
enabled = false
app_id = ""
compatdata_path = ""
proton_path = ""

[steam.launcher]
icon_path = ""
display_name = ""

[launch]
method = "native"
"#,
    )
    .unwrap();

    let loaded = store.load("legacy").unwrap();
    assert!(loaded.runtime.is_empty());
    assert_eq!(loaded.launch.method, "native");
}

#[test]
fn validate_name_rejects_invalid_names() {
    assert!(validate_name("").is_err());
    assert!(validate_name(".").is_err());
    assert!(validate_name("..").is_err());
    assert!(validate_name("foo/bar").is_err());
    assert!(validate_name("foo\\bar").is_err());
    assert!(validate_name("foo:bar").is_err());
}

#[test]
fn test_rename_success() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("old-name", &profile).unwrap();
    assert!(store.profile_path("old-name").unwrap().exists());

    store.rename("old-name", "new-name").unwrap();
    assert!(!store.profile_path("old-name").unwrap().exists());
    assert!(store.profile_path("new-name").unwrap().exists());

    let loaded = store.load("new-name").unwrap();
    assert_eq!(loaded, profile);
}

#[test]
fn test_rename_not_found() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    fs::create_dir_all(&store.base_path).unwrap();

    let result = store.rename("nonexistent", "new-name");
    assert!(result.is_err());
}

#[test]
fn test_rename_same_name() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("same-name", &profile).unwrap();

    let result = store.rename("same-name", "same-name");
    assert!(result.is_ok());
    assert!(store.profile_path("same-name").unwrap().exists());
}

#[test]
fn test_rename_preserves_content() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("original", &profile).unwrap();
    let original_content = fs::read_to_string(store.profile_path("original").unwrap()).unwrap();

    store.rename("original", "renamed").unwrap();
    let renamed_content = fs::read_to_string(store.profile_path("renamed").unwrap()).unwrap();

    assert_eq!(original_content, renamed_content);
}

#[test]
fn test_rename_rejects_existing_target_profile() {
    use crate::profile::toml_store::ProfileStoreError;

    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let source_profile = sample_profile();
    let mut target_profile = sample_profile();
    target_profile.game.name = "Different Game".to_string();

    store.save("source", &source_profile).unwrap();
    store.save("target", &target_profile).unwrap();

    let result = store.rename("source", "target");

    assert!(matches!(
        result,
        Err(ProfileStoreError::AlreadyExists(ref name)) if name == "target"
    ));
    assert!(store.profile_path("source").unwrap().exists());
    assert!(store.profile_path("target").unwrap().exists());
    assert_eq!(store.load("source").unwrap(), source_profile);
    assert_eq!(store.load("target").unwrap(), target_profile);
}
