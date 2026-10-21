//! Tests for settings merge, normalization, and restart detection.

use std::path::{Path, PathBuf};

use crosshook_core::settings::{
    AppSettingsData, ConfigHistorySettings, HighContrastPreference, ReducedMotionPreference,
    SettingsStore, CONFIG_HISTORY_MAX_REVISIONS_MAX, RECENT_FILES_LIMIT_MAX,
};
use tempfile::tempdir;

use super::{
    merge_save_request, paths_need_restart_for_profiles, SettingsSaveRequest, SettingsService,
};

fn make_save_request() -> SettingsSaveRequest {
    SettingsSaveRequest {
        auto_load_last_profile: false,
        last_used_profile: String::new(),
        community_taps: vec![],
        onboarding_completed: false,
        offline_mode: false,
        default_proton_path: String::new(),
        default_launch_method: String::new(),
        default_bundled_optimization_preset_id: String::new(),
        default_trainer_loading_mode: String::new(),
        log_filter: "info".to_string(),
        console_drawer_collapsed_default: false,
        recent_files_limit: 10,
        profiles_directory: String::new(),
        protontricks_binary_path: String::new(),
        auto_install_prefix_deps: false,
        discovery_enabled: false,
        external_trainer_sources: None,
        protonup_auto_suggest: None,
        protonup_binary_path: None,
        umu_preference: None,
        umu_database_lookup: None,
        host_tool_dashboard_dismissed_hints: None,
        host_tool_dashboard_default_category_filter: None,
        install_nag_dismissed_at: None,
        steam_deck_caveats_dismissed_at: None,
        protonup_default_provider: None,
        protonup_default_install_root: None,
        protonup_include_prereleases: None,
        high_contrast: None,
        reduced_motion: None,
        config_history: None,
    }
}

#[test]
fn paths_need_restart_table() {
    let temp = tempdir().unwrap();
    let real = temp.path().join("real");
    std::fs::create_dir_all(&real).unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let cases: [(&str, PathBuf, PathBuf, bool); 4] = [
        (
            "lexical mismatch",
            PathBuf::from("/tmp/a"),
            PathBuf::from("/tmp/b"),
            true,
        ),
        (
            "lexical equal (missing)",
            PathBuf::from("/nonexistent/x"),
            PathBuf::from("/nonexistent/x"),
            false,
        ),
        ("symlink to same dir", link.clone(), real.clone(), false),
        (
            "one missing",
            real.clone(),
            temp.path().join("missing"),
            true,
        ),
    ];
    for (label, resolved, active, expected) in cases {
        assert_eq!(
            paths_need_restart_for_profiles(&resolved, &active),
            expected,
            "{label}"
        );
    }
}

#[test]
fn merge_preserves_disk_extras_and_ignores_request_extras() {
    let current: AppSettingsData = toml::from_str(
        "future = 2026-10-08\n[ui]\nscale = 1.25\n[config_history]\nmax_revisions = 20\nfuture = true\n",
    )
    .unwrap();
    let mut request = make_save_request();
    request.offline_mode = true;
    request.config_history = Some(ConfigHistorySettings {
        max_revisions: 30,
        extra: toml::Table::from_iter([("injected".to_string(), true.into())]),
    });
    let merged = merge_save_request(request, current.clone());
    assert!(merged.offline_mode);
    assert_eq!(merged.config_history.max_revisions, 30);
    assert_eq!(merged.extra, current.extra);
    assert_eq!(merged.config_history.extra, current.config_history.extra);
}

#[test]
fn merge_preserves_steamgriddb_key() {
    let current = AppSettingsData {
        steamgriddb_api_key: Some("stored".to_string()),
        ..Default::default()
    };
    let merged = merge_save_request(make_save_request(), current);
    assert_eq!(merged.steamgriddb_api_key.as_deref(), Some("stored"));
}

/// Tri-state fields: absent (`None`) preserves, `Some(None)` clears,
/// `Some(Some(v))` sets.
#[test]
fn merge_tri_state_timestamps_table() {
    const STORED: &str = "2026-04-15T12:00:00Z";
    const NEW: &str = "2026-04-15T13:00:00Z";
    let cases: [(&str, Option<Option<String>>, Option<&str>); 3] = [
        ("absent preserves", None, Some(STORED)),
        ("explicit null clears", Some(None), None),
        ("value sets", Some(Some(NEW.to_string())), Some(NEW)),
    ];
    for (label, input, expected) in cases {
        let current = AppSettingsData {
            install_nag_dismissed_at: Some(STORED.to_string()),
            steam_deck_caveats_dismissed_at: Some(STORED.to_string()),
            host_tool_dashboard_default_category_filter: Some(STORED.to_string()),
            ..Default::default()
        };
        let mut request = make_save_request();
        request.install_nag_dismissed_at = input.clone();
        request.steam_deck_caveats_dismissed_at = input.clone();
        request.host_tool_dashboard_default_category_filter = input;
        let merged = merge_save_request(request, current);
        assert_eq!(
            merged.install_nag_dismissed_at.as_deref(),
            expected,
            "install_nag: {label}"
        );
        assert_eq!(
            merged.steam_deck_caveats_dismissed_at.as_deref(),
            expected,
            "steam_deck: {label}"
        );
        assert_eq!(
            merged
                .host_tool_dashboard_default_category_filter
                .as_deref(),
            expected,
            "category_filter: {label}"
        );
    }
}

#[test]
fn merge_accessibility_preferences_table() {
    let cases = [
        (
            "absent preserves",
            None,
            None,
            HighContrastPreference::On,
            ReducedMotionPreference::Reduced,
        ),
        (
            "explicit sets",
            Some(HighContrastPreference::Off),
            Some(ReducedMotionPreference::Full),
            HighContrastPreference::Off,
            ReducedMotionPreference::Full,
        ),
    ];
    for (label, high_contrast, reduced_motion, want_hc, want_rm) in cases {
        let current = AppSettingsData {
            high_contrast: HighContrastPreference::On,
            reduced_motion: ReducedMotionPreference::Reduced,
            ..Default::default()
        };
        let mut request = make_save_request();
        request.high_contrast = high_contrast;
        request.reduced_motion = reduced_motion;
        let merged = merge_save_request(request, current);
        assert_eq!(merged.high_contrast, want_hc, "{label}");
        assert_eq!(merged.reduced_motion, want_rm, "{label}");
    }
}

#[test]
fn merge_normalizes_limits_and_log_filter() {
    let mut request = make_save_request();
    request.recent_files_limit = 0;
    request.log_filter = "   ".to_string();
    request.config_history = Some(ConfigHistorySettings {
        max_revisions: u32::MAX,
        extra: toml::Table::new(),
    });
    let merged = merge_save_request(request, AppSettingsData::default());
    assert_eq!(merged.recent_files_limit, 1);
    assert_eq!(merged.log_filter, "info");
    assert_eq!(
        merged.config_history.max_revisions,
        CONFIG_HISTORY_MAX_REVISIONS_MAX
    );
}

#[test]
fn save_request_deserializes_tri_state_strings() {
    let mut body = serde_json::to_value(make_save_request()).expect("fixture must serialize");
    let map = body.as_object_mut().expect("request body is a JSON object");
    map.insert("high_contrast".to_string(), serde_json::json!("off"));
    map.insert("reduced_motion".to_string(), serde_json::json!("reduced"));
    map.insert(
        "install_nag_dismissed_at".to_string(),
        serde_json::Value::Null,
    );
    let parsed: SettingsSaveRequest =
        serde_json::from_value(body).expect("tri-state strings must deserialize");
    assert_eq!(parsed.high_contrast, Some(HighContrastPreference::Off));
    assert_eq!(
        parsed.reduced_motion,
        Some(ReducedMotionPreference::Reduced)
    );

    let mut body = serde_json::to_value(make_save_request()).expect("fixture must serialize");
    let map = body.as_object_mut().expect("request body is a JSON object");
    map.remove("high_contrast");
    map.remove("reduced_motion");
    map.remove("install_nag_dismissed_at");
    let parsed: SettingsSaveRequest =
        serde_json::from_value(body).expect("absent tri-state keys must deserialize");
    assert_eq!(parsed.high_contrast, None);
    assert_eq!(parsed.reduced_motion, None);
    assert_eq!(parsed.install_nag_dismissed_at, None);
}

#[test]
fn save_request_rejects_unknown_tri_state_string() {
    let mut body = serde_json::to_value(make_save_request()).expect("fixture must serialize");
    let map = body.as_object_mut().expect("request body is a JSON object");
    map.insert("high_contrast".to_string(), serde_json::json!("yes"));
    assert!(serde_json::from_value::<SettingsSaveRequest>(body).is_err());
}

fn service_in(dir: &Path, active: PathBuf) -> (SettingsService, SettingsStore) {
    let store = SettingsStore::with_base_path(dir.to_path_buf());
    (SettingsService::new(store.clone(), active), store)
}

#[tokio::test]
async fn update_normalizes_and_preserves_unknown_keys() {
    let temp = tempdir().unwrap();
    std::fs::write(
        temp.path().join("settings.toml"),
        "future_key = \"kept\"\nlog_filter = \"debug\"\n",
    )
    .unwrap();
    let (service, store) = service_in(temp.path(), temp.path().join("profiles"));
    let saved = service
        .update(|settings| {
            settings.recent_files_limit = 10_000;
            settings.log_filter = String::new();
            settings.config_history.max_revisions = 0;
        })
        .await
        .unwrap();
    assert_eq!(saved.recent_files_limit, RECENT_FILES_LIMIT_MAX);
    assert_eq!(saved.log_filter, "info");
    assert_eq!(saved.config_history.max_revisions, 5);
    let on_disk = std::fs::read_to_string(store.settings_path()).unwrap();
    assert!(on_disk.contains("future_key = \"kept\""));
    assert_eq!(store.load().unwrap(), saved);
}

#[tokio::test]
async fn save_steamgriddb_key_trims_and_clears() {
    let temp = tempdir().unwrap();
    let (service, store) = service_in(temp.path(), temp.path().join("profiles"));
    service
        .save_steamgriddb_key(Some("  abc  ".to_string()))
        .await
        .unwrap();
    assert_eq!(
        store.load().unwrap().steamgriddb_api_key.as_deref(),
        Some("abc")
    );
    service
        .save_steamgriddb_key(Some("   ".to_string()))
        .await
        .unwrap();
    assert_eq!(store.load().unwrap().steamgriddb_api_key, None);
}

#[tokio::test]
async fn save_request_keeps_key_and_load_reports_restart() {
    let temp = tempdir().unwrap();
    let active = temp.path().join("profiles");
    let (service, _store) = service_in(temp.path(), active.clone());
    service
        .save_steamgriddb_key(Some("key".to_string()))
        .await
        .unwrap();

    let snapshot = service.load().await.unwrap();
    assert_eq!(snapshot.resolved_profiles_directory, active);
    assert_eq!(snapshot.active_profiles_directory, active);
    assert!(!snapshot.profiles_directory_requires_restart);

    let mut request = make_save_request();
    let other = temp.path().join("elsewhere");
    request.profiles_directory = other.display().to_string();
    service.save_request(request).await.unwrap();

    let snapshot = service.load().await.unwrap();
    assert_eq!(snapshot.data.steamgriddb_api_key.as_deref(), Some("key"));
    assert_eq!(snapshot.resolved_profiles_directory, other);
    assert!(snapshot.profiles_directory_requires_restart);
}
