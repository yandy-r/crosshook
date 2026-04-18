use std::collections::BTreeMap;
use std::fs;

use tempfile::tempdir;

use crate::profile::models::LaunchOptimizationsSection;
use crate::profile::toml_store::{ProfileStore, ProfileStoreError};

use super::fixtures::sample_profile;

#[test]
fn save_launch_optimizations_merges_only_launch_section() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();

    let optimizations = LaunchOptimizationsSection {
        enabled_option_ids: vec![
            "disable_steam_input".to_string(),
            "use_gamemode".to_string(),
        ],
    };
    store
        .save_launch_optimizations("elden-ring", optimizations.enabled_option_ids.clone(), None)
        .unwrap();

    let loaded = store.load("elden-ring").unwrap();
    assert_eq!(loaded.game, profile.game);
    assert_eq!(loaded.trainer, profile.trainer);
    assert_eq!(loaded.injection, profile.injection);
    assert_eq!(loaded.steam, profile.steam);
    assert_eq!(loaded.runtime, profile.runtime);
    assert_eq!(loaded.launch.method, profile.launch.method);
    assert_eq!(loaded.launch.optimizations, optimizations);
}

#[test]
fn save_launch_optimizations_rejects_missing_profiles() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

    let result =
        store.save_launch_optimizations("missing-profile", vec!["use_gamemode".to_string()], None);

    assert!(matches!(result, Err(ProfileStoreError::NotFound(_))));
}

#[test]
fn save_launch_optimizations_rejects_unknown_option_ids() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();

    let result = store.save_launch_optimizations(
        "elden-ring",
        vec!["not_a_real_launch_optimization".to_string()],
        None,
    );

    assert!(matches!(
        result,
        Err(ProfileStoreError::InvalidLaunchOptimizationId(id)) if id == "not_a_real_launch_optimization"
    ));
}

#[test]
fn load_normalizes_optimizations_from_active_preset() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    fs::create_dir_all(&store.base_path).unwrap();

    let toml = r#"[game]
name = "Test"
executable_path = "/games/test.exe"

[trainer]
path = ""
type = ""
loading_mode = "source_directory"

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

[runtime]
prefix_path = ""
proton_path = ""
working_directory = ""

[launch]
method = "proton_run"
active_preset = "performance"

[launch.optimizations]
enabled_option_ids = ["enable_hdr"]

[launch.presets.performance]
enabled_option_ids = ["use_gamemode", "disable_steam_input"]

[launch.presets.quality]
enabled_option_ids = ["enable_hdr"]
"#;
    fs::write(store.profile_path("preset-test").unwrap(), toml).unwrap();

    let loaded = store.load("preset-test").unwrap();
    assert_eq!(loaded.launch.active_preset, "performance");
    assert_eq!(
        loaded.launch.optimizations.enabled_option_ids,
        vec![
            "use_gamemode".to_string(),
            "disable_steam_input".to_string()
        ]
    );
    let mut expected = BTreeMap::new();
    expected.insert(
        "performance".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec![
                "use_gamemode".to_string(),
                "disable_steam_input".to_string(),
            ],
        },
    );
    expected.insert(
        "quality".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["enable_hdr".to_string()],
        },
    );
    assert_eq!(loaded.launch.presets, expected);
}

#[test]
fn save_launch_optimizations_updates_active_preset_entry() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();

    let mut presets = BTreeMap::new();
    presets.insert(
        "a".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["use_gamemode".to_string()],
        },
    );
    profile.launch.presets = presets;
    profile.launch.active_preset = "a".to_string();
    profile.launch.optimizations = profile.launch.presets["a"].clone();

    store.save("p", &profile).unwrap();

    store
        .save_launch_optimizations(
            "p",
            vec!["use_ntsync".to_string(), "disable_esync".to_string()],
            None,
        )
        .unwrap();

    let loaded = store.load("p").unwrap();
    assert_eq!(loaded.launch.active_preset, "a");
    assert_eq!(
        loaded.launch.optimizations.enabled_option_ids,
        vec!["use_ntsync".to_string(), "disable_esync".to_string()]
    );
    assert_eq!(
        loaded.launch.presets["a"].enabled_option_ids,
        vec!["use_ntsync".to_string(), "disable_esync".to_string()]
    );
}

#[test]
fn materialize_launch_optimization_preset_sets_active_and_presets() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    store.save("p", &profile).unwrap();

    let ids = vec!["use_gamemode".to_string(), "enable_nvapi".to_string()];
    store
        .materialize_launch_optimization_preset(
            "p",
            "bundled/nvidia_performance",
            ids.clone(),
            true,
        )
        .unwrap();

    let loaded = store.load("p").unwrap();
    assert_eq!(loaded.launch.active_preset, "bundled/nvidia_performance");
    assert_eq!(loaded.launch.optimizations.enabled_option_ids, ids);
    assert_eq!(
        loaded
            .launch
            .presets
            .get("bundled/nvidia_performance")
            .unwrap()
            .enabled_option_ids,
        ids
    );
}

#[test]
fn save_launch_optimizations_switch_active_preset() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();

    let mut presets = BTreeMap::new();
    presets.insert(
        "a".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["use_gamemode".to_string()],
        },
    );
    presets.insert(
        "b".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["enable_hdr".to_string()],
        },
    );
    profile.launch.presets = presets;
    profile.launch.active_preset = "a".to_string();
    profile.launch.optimizations = profile.launch.presets["a"].clone();

    store.save("p", &profile).unwrap();

    store
        .save_launch_optimizations("p", vec![], Some("b".to_string()))
        .unwrap();

    let loaded = store.load("p").unwrap();
    assert_eq!(loaded.launch.active_preset, "b");
    assert_eq!(
        loaded.launch.optimizations.enabled_option_ids,
        vec!["enable_hdr".to_string()]
    );
}

#[test]
fn save_launch_optimizations_rejects_missing_preset_name() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("p", &sample_profile()).unwrap();

    let result = store.save_launch_optimizations("p", vec![], Some("nope".to_string()));

    assert!(matches!(
        result,
        Err(ProfileStoreError::LaunchPresetNotFound(name)) if name == "nope"
    ));
}
