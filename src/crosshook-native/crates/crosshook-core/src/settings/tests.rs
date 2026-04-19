use super::*;
use crate::community::CommunityTapSubscription;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn load_returns_default_settings_when_file_is_missing() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

    let settings = store.load().unwrap();

    assert_eq!(settings, AppSettingsData::default());
    assert!(store.settings_path().parent().unwrap().exists());
}

#[test]
fn save_and_load_round_trip() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
    let settings = AppSettingsData {
        auto_load_last_profile: true,
        last_used_profile: "elden-ring".to_string(),
        community_taps: vec![CommunityTapSubscription {
            url: "https://example.invalid/community.git".to_string(),
            branch: Some("main".to_string()),
            pinned_commit: Some("deadbeef".to_string()),
        }],
        onboarding_completed: true,
        offline_mode: false,
        steamgriddb_api_key: None,
        ..Default::default()
    };

    store.save(&settings).unwrap();

    assert_eq!(store.load().unwrap(), settings);
    assert!(store.settings_path().exists());
}

#[test]
fn onboarding_completed_defaults_to_false_when_absent() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

    fs::create_dir_all(&store.base_path).unwrap();
    // TOML that deliberately omits onboarding_completed
    fs::write(
        store.settings_path(),
        "auto_load_last_profile = true\nlast_used_profile = \"elden-ring\"\n",
    )
    .unwrap();

    let settings = store.load().unwrap();
    assert!(!settings.onboarding_completed);
}

#[test]
fn offline_mode_defaults_false_when_absent() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

    fs::create_dir_all(&store.base_path).unwrap();
    fs::write(
        store.settings_path(),
        "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
    )
    .unwrap();

    let settings = store.load().unwrap();
    assert!(!settings.offline_mode);
}

#[test]
fn high_contrast_defaults_false_when_absent() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

    fs::create_dir_all(&store.base_path).unwrap();
    fs::write(
        store.settings_path(),
        "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
    )
    .unwrap();

    let settings = store.load().unwrap();
    assert!(
        !settings.high_contrast,
        "high_contrast should default to false when not present in settings.toml"
    );
}

#[test]
fn load_uses_missing_fields_defaults() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));

    fs::create_dir_all(&store.base_path).unwrap();
    fs::write(
        store.settings_path(),
        "last_used_profile = \"elden-ring\"\n",
    )
    .unwrap();

    let settings = store.load().unwrap();

    assert_eq!(
        settings,
        AppSettingsData {
            auto_load_last_profile: false,
            last_used_profile: "elden-ring".to_string(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            steamgriddb_api_key: None,
            ..Default::default()
        },
    );
}

#[test]
fn resolve_profiles_directory_default_under_config() {
    let temp = tempdir().unwrap();
    let cfg = temp.path().join("crosshook");
    let s = AppSettingsData::default();
    let p = resolve_profiles_directory_from_config(&s, &cfg).unwrap();
    assert_eq!(p, cfg.join("profiles"));
}

#[test]
fn resolve_profiles_directory_custom_tilde() {
    let temp = tempdir().unwrap();
    let cfg = temp.path().join("crosshook");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    // Pretend home via env is not used — expand_path uses BaseDirs which uses real home.
    // Test only default branch; tilde test in integration if needed.
    let s = AppSettingsData {
        profiles_directory: temp.path().join("myprofiles").display().to_string(),
        ..Default::default()
    };
    let p = resolve_profiles_directory_from_config(&s, &cfg).unwrap();
    assert_eq!(p, PathBuf::from(s.profiles_directory));
}

#[test]
fn settings_roundtrip_with_protontricks_fields() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
    let settings = AppSettingsData {
        protontricks_binary_path: "/usr/bin/protontricks".to_string(),
        auto_install_prefix_deps: true,
        ..Default::default()
    };
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.protontricks_binary_path, "/usr/bin/protontricks");
    assert!(loaded.auto_install_prefix_deps);
}

#[test]
fn settings_backward_compat_without_protontricks_fields() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
    // Save settings without the new fields (simulate old config)
    let old_toml = "auto_load_last_profile = false\nlast_used_profile = \"\"\n";
    std::fs::create_dir_all(store.settings_path().parent().unwrap()).unwrap();
    std::fs::write(store.settings_path(), old_toml).unwrap();
    let loaded = store.load().unwrap();
    assert!(loaded.protontricks_binary_path.is_empty());
    assert!(!loaded.auto_install_prefix_deps);
}

#[test]
fn settings_backward_compat_without_protonup_fields() {
    let temp_dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
    // Old TOML that has no protonup_* keys — new fields must fall back to defaults.
    let old_toml = "auto_load_last_profile = false\nlast_used_profile = \"\"\n";
    std::fs::create_dir_all(store.settings_path().parent().unwrap()).unwrap();
    std::fs::write(store.settings_path(), old_toml).unwrap();
    let loaded = store.load().unwrap();
    assert!(
        loaded.protonup_auto_suggest,
        "protonup_auto_suggest should default to true"
    );
    assert!(
        loaded.protonup_binary_path.is_empty(),
        "protonup_binary_path should default to empty"
    );
}

#[test]
fn settings_backward_compat_without_protonup_manager_fields() {
    // TOML that has pre-v22 settings including protonup_auto_suggest / protonup_binary_path
    // but NOT the three new manager fields. Must parse cleanly and fill defaults.
    let legacy_toml = r#"
protonup_auto_suggest = true
protonup_binary_path = ""
# deliberately omit protonup_default_provider, protonup_default_install_root, protonup_include_prereleases
        "#;
    let parsed: AppSettingsData = toml::from_str(legacy_toml).expect("parses legacy toml");
    assert_eq!(parsed.protonup_default_provider, "ge-proton");
    assert_eq!(parsed.protonup_default_install_root, "");
    assert!(!parsed.protonup_include_prereleases);
}

#[test]
fn settings_roundtrip_protonup_manager_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let settings = AppSettingsData {
        protonup_default_provider: "proton-cachyos".to_string(),
        protonup_default_install_root: "/home/user/.steam/root/compatibilitytools.d".to_string(),
        protonup_include_prereleases: true,
        ..Default::default()
    };
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.protonup_default_provider, "proton-cachyos");
    assert_eq!(
        loaded.protonup_default_install_root,
        "/home/user/.steam/root/compatibilitytools.d"
    );
    assert!(loaded.protonup_include_prereleases);
}

#[test]
fn settings_backward_compat_without_umu_preference() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let path = store.settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
    )
    .unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.umu_preference, UmuPreference::Auto);
}

#[test]
fn settings_backward_compat_without_install_nag_dismissed_at() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let path = store.settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
    )
    .unwrap();
    let loaded = store.load().unwrap();
    assert!(
        loaded.install_nag_dismissed_at.is_none(),
        "install_nag_dismissed_at should default to None when absent from settings.toml"
    );
}

#[test]
fn settings_save_roundtrip_preserves_install_nag_dismissed_at() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let timestamp = "2026-04-15T12:00:00Z".to_string();
    let settings = AppSettingsData {
        install_nag_dismissed_at: Some(timestamp.clone()),
        ..Default::default()
    };
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(
        loaded.install_nag_dismissed_at,
        Some(timestamp),
        "install_nag_dismissed_at must survive a save/load roundtrip"
    );
}

#[test]
fn settings_backward_compat_without_steam_deck_caveats_dismissed_at() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let path = store.settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
    )
    .unwrap();
    let loaded = store.load().unwrap();
    assert!(
        loaded.steam_deck_caveats_dismissed_at.is_none(),
        "steam_deck_caveats_dismissed_at should default to None when absent from settings.toml"
    );
}

#[test]
fn settings_roundtrip_steam_deck_caveats_dismissed_at() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let timestamp = "2026-04-15T12:00:00Z".to_string();
    let settings = AppSettingsData {
        steam_deck_caveats_dismissed_at: Some(timestamp.clone()),
        ..Default::default()
    };
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(
        loaded.steam_deck_caveats_dismissed_at,
        Some(timestamp),
        "steam_deck_caveats_dismissed_at must survive a save/load roundtrip"
    );
}

#[test]
fn settings_roundtrip_umu_preference_umu() {
    let toml = "umu_preference = \"umu\"\n";
    let parsed: AppSettingsData = toml::from_str(toml).unwrap();
    assert_eq!(parsed.umu_preference, UmuPreference::Umu);
    let serialized = toml::to_string(&parsed).unwrap();
    assert!(serialized.contains("umu_preference = \"umu\""));
}

#[test]
fn settings_backward_compat_without_host_tool_dashboard_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let path = store.settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "auto_load_last_profile = false\nlast_used_profile = \"\"\n",
    )
    .unwrap();
    let loaded = store.load().unwrap();
    assert!(loaded.host_tool_dashboard_dismissed_hints.is_empty());
    assert!(loaded.host_tool_dashboard_default_category_filter.is_none());
}

#[test]
fn settings_roundtrip_host_tool_dashboard_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().to_path_buf());
    let settings = AppSettingsData {
        host_tool_dashboard_dismissed_hints: vec![
            "gamescope".to_string(),
            "prefix_tools".to_string(),
        ],
        host_tool_dashboard_default_category_filter: Some("runtime".to_string()),
        ..Default::default()
    };
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(
        loaded.host_tool_dashboard_dismissed_hints,
        vec!["gamescope".to_string(), "prefix_tools".to_string()]
    );
    assert_eq!(
        loaded.host_tool_dashboard_default_category_filter,
        Some("runtime".to_string())
    );
}

#[test]
fn umu_preference_from_str_rejects_unknown() {
    use std::str::FromStr;
    assert!(UmuPreference::from_str("ghoti").is_err());
    assert_eq!(UmuPreference::from_str("umu").unwrap(), UmuPreference::Umu);
    assert_eq!(
        UmuPreference::from_str("auto").unwrap(),
        UmuPreference::Auto
    );
    assert_eq!(
        UmuPreference::from_str("proton").unwrap(),
        UmuPreference::Proton
    );
}
