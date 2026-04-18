#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn effective_profile_prefers_local_override_paths() {
    let mut profile = sample_profile();
    profile.game.executable_path = "/portable/game.exe".to_string();
    profile.local_override.game.executable_path = "/local/game.exe".to_string();
    profile.runtime.proton_path = "/portable/proton".to_string();
    profile.local_override.runtime.proton_path = "/local/proton".to_string();

    let effective = profile.effective_profile();
    assert_eq!(effective.game.executable_path, "/local/game.exe");
    assert_eq!(effective.runtime.proton_path, "/local/proton");
}

#[test]
fn storage_profile_moves_machine_paths_to_local_override() {
    let mut profile = sample_profile();
    profile.game.executable_path = "/games/test.exe".to_string();
    profile.trainer.path = "/trainers/test.exe".to_string();
    profile.steam.compatdata_path = "/steam/compatdata/123".to_string();
    profile.steam.proton_path = "/steam/proton/proton".to_string();
    profile.runtime.prefix_path = "/prefix/123".to_string();
    profile.runtime.proton_path = "/runtime/proton".to_string();

    let storage = profile.storage_profile();
    assert_eq!(storage.game.executable_path, "");
    assert_eq!(storage.trainer.path, "");
    assert_eq!(storage.steam.compatdata_path, "");
    assert_eq!(storage.steam.proton_path, "");
    assert_eq!(storage.runtime.prefix_path, "");
    assert_eq!(storage.runtime.proton_path, "");
    assert_eq!(
        storage.local_override.game.executable_path,
        "/games/test.exe"
    );
    assert_eq!(storage.local_override.trainer.path, "/trainers/test.exe");
    assert_eq!(
        storage.local_override.steam.compatdata_path,
        "/steam/compatdata/123"
    );
    assert_eq!(
        storage.local_override.steam.proton_path,
        "/steam/proton/proton"
    );
    assert_eq!(storage.local_override.runtime.prefix_path, "/prefix/123");
    assert_eq!(
        storage.local_override.runtime.proton_path,
        "/runtime/proton"
    );
}

#[test]
fn portable_profile_clears_local_override_fields() {
    let mut profile = sample_profile();
    profile.game.executable_path = "/games/test.exe".to_string();
    profile.trainer.path = "/trainers/test.exe".to_string();
    profile.steam.compatdata_path = "/steam/compatdata/123".to_string();
    profile.steam.proton_path = "/steam/proton/proton".to_string();
    profile.runtime.prefix_path = "/prefix/123".to_string();
    profile.runtime.proton_path = "/runtime/proton".to_string();

    let portable = profile.portable_profile();
    assert_eq!(portable.local_override.game.executable_path, "");
    assert_eq!(portable.local_override.trainer.path, "");
    assert_eq!(portable.local_override.steam.compatdata_path, "");
    assert_eq!(portable.local_override.steam.proton_path, "");
    assert_eq!(portable.local_override.runtime.prefix_path, "");
    assert_eq!(portable.local_override.runtime.proton_path, "");
}

#[test]
fn storage_profile_roundtrip_is_idempotent() {
    let mut profile = sample_profile();
    profile.game.executable_path = "/games/test.exe".to_string();
    profile.trainer.path = "/trainers/test.exe".to_string();
    profile.steam.compatdata_path = "/steam/compatdata/123".to_string();
    profile.steam.proton_path = "/steam/proton/proton".to_string();
    profile.runtime.prefix_path = "/prefix/123".to_string();
    profile.runtime.proton_path = "/runtime/proton".to_string();

    let storage_once = profile.storage_profile();
    let storage_twice = storage_once.effective_profile().storage_profile();
    assert_eq!(storage_twice, storage_once);
}

#[test]
fn effective_profile_with_none_equals_shim() {
    let mut profile = sample_profile();
    profile
        .launch
        .custom_env_vars
        .insert("DXVK_HUD".to_string(), "1".to_string());
    profile.local_override.game.executable_path = "/local/game.exe".to_string();

    let via_shim = profile.effective_profile();
    let via_with_none = profile.effective_profile_with(None);
    assert_eq!(via_shim, via_with_none, "shim must equal explicit None");
}

#[test]
fn effective_profile_with_merges_collection_defaults_between_base_and_local_override() {
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();
    profile
        .launch
        .custom_env_vars
        .insert("PROFILE_ONLY".to_string(), "A".to_string());
    profile.game.executable_path = "/portable/game.exe".to_string();
    profile.local_override.game.executable_path = "/local/game.exe".to_string();

    let mut defaults = CollectionDefaultsSection::default();
    defaults
        .custom_env_vars
        .insert("COLLECTION_ONLY".to_string(), "B".to_string());
    defaults
        .custom_env_vars
        .insert("PROFILE_ONLY".to_string(), "OVERRIDDEN".to_string());
    defaults.network_isolation = Some(false);
    defaults.method = Some("proton_run".to_string());

    let merged = profile.effective_profile_with(Some(&defaults));

    // ── Layer 3 (local_override) still wins last ──
    assert_eq!(merged.game.executable_path, "/local/game.exe");

    // ── Layer 2 (collection defaults) applies ──
    assert_eq!(merged.launch.method, "proton_run");
    assert!(!merged.launch.network_isolation);
    assert_eq!(
        merged
            .launch
            .custom_env_vars
            .get("COLLECTION_ONLY")
            .cloned(),
        Some("B".to_string())
    );
    // ── Collection key wins on collision ──
    assert_eq!(
        merged.launch.custom_env_vars.get("PROFILE_ONLY").cloned(),
        Some("OVERRIDDEN".to_string())
    );
}

#[test]
fn effective_profile_with_none_fields_do_not_overwrite_profile() {
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();
    profile.launch.network_isolation = true;
    profile.launch.gamescope = GamescopeConfig::default();
    profile
        .launch
        .custom_env_vars
        .insert("PROFILE_KEY".to_string(), "retained".to_string());

    // Empty defaults: every Option is None, BTreeMap is empty → no-op merge.
    let defaults = CollectionDefaultsSection::default();
    assert!(defaults.is_empty());
    let merged = profile.effective_profile_with(Some(&defaults));

    assert_eq!(merged.launch.method, "native");
    assert!(merged.launch.network_isolation);
    assert_eq!(
        merged.launch.custom_env_vars.get("PROFILE_KEY").cloned(),
        Some("retained".to_string())
    );
    // ── Profile env vars never dropped ──
    assert_eq!(merged.launch.custom_env_vars.len(), 1);
}

#[test]
fn effective_profile_with_ignores_whitespace_only_method() {
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();

    let mut defaults = CollectionDefaultsSection::default();
    defaults.method = Some("   ".to_string()); // whitespace-only must NOT clobber profile

    let merged = profile.effective_profile_with(Some(&defaults));
    assert_eq!(
        merged.launch.method, "native",
        "whitespace method must not clobber profile"
    );
}
