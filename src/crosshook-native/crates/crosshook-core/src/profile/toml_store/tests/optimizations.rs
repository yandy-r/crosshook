use std::collections::BTreeMap;
use std::fs;

use tempfile::tempdir;

use crate::profile::models::LaunchOptimizationsSection;
use crate::profile::toml_store::{ProfileStore, ProfileStoreError};
use crate::profile::GameProfile;

use super::fixtures::sample_profile;

/// Loads `name`, applies `mutate`, and saves once — the writer's mutate-then-persist sequence.
fn load_apply_save(
    store: &ProfileStore,
    name: &str,
    mutate: impl FnOnce(&mut GameProfile) -> Result<(), ProfileStoreError>,
) -> Result<(), ProfileStoreError> {
    let mut profile = store.load(name)?;
    mutate(&mut profile)?;
    store.save(name, &profile)
}

#[test]
fn apply_launch_optimizations_persists_only_launch_section() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();

    let optimizations = LaunchOptimizationsSection {
        extra: toml::Table::new(),
        enabled_option_ids: vec![
            "disable_steam_input".to_string(),
            "use_gamemode".to_string(),
        ],
    };
    let ids = optimizations.enabled_option_ids.clone();
    load_apply_save(&store, "elden-ring", |p| {
        p.apply_launch_optimizations(ids, None)
    })
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
fn apply_launch_optimizations_trims_and_drops_blank_ids() {
    let mut profile = sample_profile();

    profile
        .apply_launch_optimizations(vec!["  use_ntsync ".to_string(), String::new()], None)
        .unwrap();

    assert_eq!(
        profile.launch.optimizations.enabled_option_ids,
        vec!["use_ntsync".to_string()]
    );
}

#[test]
fn apply_launch_optimizations_rejects_unknown_ids_without_mutating() {
    let mut profile = sample_profile();
    let before = profile.clone();

    let result = profile
        .apply_launch_optimizations(vec!["not_a_real_launch_optimization".to_string()], None);

    assert!(matches!(
        result,
        Err(ProfileStoreError::InvalidLaunchOptimizationId(id)) if id == "not_a_real_launch_optimization"
    ));
    assert_eq!(profile, before);
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
            extra: toml::Table::new(),
            enabled_option_ids: vec![
                "use_gamemode".to_string(),
                "disable_steam_input".to_string(),
            ],
        },
    );
    expected.insert(
        "quality".to_string(),
        LaunchOptimizationsSection {
            extra: toml::Table::new(),
            enabled_option_ids: vec!["enable_hdr".to_string()],
        },
    );
    assert_eq!(loaded.launch.presets, expected);
}

#[test]
fn apply_launch_optimizations_updates_active_preset_entry() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();

    let mut presets = BTreeMap::new();
    presets.insert(
        "a".to_string(),
        LaunchOptimizationsSection {
            extra: toml::Table::new(),
            enabled_option_ids: vec!["use_gamemode".to_string()],
        },
    );
    profile.launch.presets = presets;
    profile.launch.active_preset = "a".to_string();
    profile.launch.optimizations = profile.launch.presets["a"].clone();

    store.save("p", &profile).unwrap();

    load_apply_save(&store, "p", |p| {
        p.apply_launch_optimizations(
            vec!["use_ntsync".to_string(), "disable_esync".to_string()],
            None,
        )
    })
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
    let applied = ids.clone();
    load_apply_save(&store, "p", |p| {
        p.materialize_launch_optimization_preset("bundled/nvidia_performance", applied, true)
    })
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
fn materialize_launch_optimization_preset_without_activation_keeps_active_selection() {
    let mut profile = sample_profile();
    profile.launch.optimizations.enabled_option_ids = vec!["enable_hdr".to_string()];

    profile
        .materialize_launch_optimization_preset("  saved  ", vec!["use_gamemode".into()], false)
        .unwrap();

    assert_eq!(profile.launch.active_preset, "");
    assert_eq!(
        profile.launch.optimizations.enabled_option_ids,
        vec!["enable_hdr".to_string()]
    );
    assert_eq!(
        profile.launch.presets["saved"].enabled_option_ids,
        vec!["use_gamemode".to_string()]
    );
}

#[test]
fn materialize_launch_optimization_preset_rejects_invalid_input_without_mutating() {
    let mut profile = sample_profile();
    let before = profile.clone();

    assert!(matches!(
        profile.materialize_launch_optimization_preset("k", vec!["nope".into()], true),
        Err(ProfileStoreError::InvalidLaunchOptimizationId(id)) if id == "nope"
    ));
    assert!(matches!(
        profile.materialize_launch_optimization_preset("   ", vec![], true),
        Err(ProfileStoreError::LaunchPresetNotFound(key)) if key == "   "
    ));
    assert_eq!(profile, before);
}

#[test]
fn apply_launch_optimizations_switch_active_preset() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let mut profile = sample_profile();

    let mut presets = BTreeMap::new();
    presets.insert(
        "a".to_string(),
        LaunchOptimizationsSection {
            extra: toml::Table::new(),
            enabled_option_ids: vec!["use_gamemode".to_string()],
        },
    );
    presets.insert(
        "b".to_string(),
        LaunchOptimizationsSection {
            extra: toml::Table::new(),
            enabled_option_ids: vec!["enable_hdr".to_string()],
        },
    );
    profile.launch.presets = presets;
    profile.launch.active_preset = "a".to_string();
    profile.launch.optimizations = profile.launch.presets["a"].clone();

    store.save("p", &profile).unwrap();

    load_apply_save(&store, "p", |p| {
        p.apply_launch_optimizations(vec![], Some("b".to_string()))
    })
    .unwrap();

    let loaded = store.load("p").unwrap();
    assert_eq!(loaded.launch.active_preset, "b");
    assert_eq!(
        loaded.launch.optimizations.enabled_option_ids,
        vec!["enable_hdr".to_string()]
    );
}

#[test]
fn apply_launch_optimizations_rejects_missing_preset_name() {
    let mut profile = sample_profile();
    let before = profile.clone();

    let result = profile.apply_launch_optimizations(vec![], Some("nope".to_string()));
    assert!(matches!(
        result,
        Err(ProfileStoreError::LaunchPresetNotFound(name)) if name == "nope"
    ));

    let result = profile.apply_launch_optimizations(vec![], Some("  ".to_string()));
    assert!(matches!(
        result,
        Err(ProfileStoreError::LaunchPresetNotFound(name)) if name == "  "
    ));
    assert_eq!(profile, before);
}

#[test]
fn apply_launch_optimizations_rejects_switch_to_preset_with_unknown_ids() {
    let mut profile = sample_profile();
    profile.launch.presets.insert(
        "stale".into(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["retired_option".into()],
            extra: toml::Table::new(),
        },
    );
    let before = profile.clone();

    let result = profile.apply_launch_optimizations(vec![], Some("stale".into()));

    assert!(matches!(
        result,
        Err(ProfileStoreError::InvalidLaunchOptimizationId(id)) if id == "retired_option"
    ));
    assert_eq!(profile, before);
}

#[test]
fn switch_preset_preserves_active_and_preset_extras_after_reload() {
    let dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(dir.path().join("profiles"));
    let mut profile = sample_profile();
    profile.launch.optimizations.extra = toml::Table::from_iter([
        ("future_active".into(), 1.25.into()),
        ("shared".into(), "active".into()),
    ]);
    profile.launch.presets.insert(
        "other".into(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["use_gamemode".into()],
            extra: toml::Table::from_iter([
                ("future_preset".into(), true.into()),
                ("shared".into(), "preset".into()),
            ]),
        },
    );
    store.save("game", &profile).unwrap();
    load_apply_save(&store, "game", |p| {
        p.apply_launch_optimizations(vec![], Some("other".into()))
    })
    .unwrap();

    let loaded = store.load("game").unwrap();
    assert_eq!(
        loaded.launch.optimizations.extra["future_active"].as_float(),
        Some(1.25)
    );
    assert!(!loaded
        .launch
        .optimizations
        .extra
        .contains_key("future_preset"));
    assert_eq!(
        loaded.launch.optimizations.extra["shared"].as_str(),
        Some("active")
    );
    assert_eq!(
        loaded.launch.presets["other"],
        profile.launch.presets["other"]
    );
}

#[test]
fn apply_manual_optimization_preset_activates_trimmed_name_in_memory() {
    let mut profile = sample_profile();

    profile
        .apply_manual_optimization_preset("  Mine  ", vec!["use_gamemode".into()])
        .unwrap();

    assert_eq!(profile.launch.active_preset, "Mine");
    assert_eq!(
        profile.launch.optimizations.enabled_option_ids,
        vec!["use_gamemode".to_string()]
    );
    assert_eq!(
        profile.launch.presets["Mine"].enabled_option_ids,
        vec!["use_gamemode".to_string()]
    );
}
