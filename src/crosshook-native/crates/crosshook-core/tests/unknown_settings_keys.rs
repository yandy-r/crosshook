use crosshook_core::settings::{AppSettingsData, RecentFilesData, RecentFilesStore, SettingsStore};
use std::fs;
use tempfile::tempdir;

const SETTINGS: &str = r#"
future_float = 1.25
future_date = 2026-10-08T12:30:00Z
[ui]
library_view_mode = "grid"
scale = 1.1
density = "compact"
renderer = "auto"
controller_mode = true
window_width = 1200
window_height = 800
inspector_width = 300
console_height = 220
sidebar_collapsed = false
palette_recent = ["launch"]
[config_history]
max_revisions = 20
future = { nested = [1, 2] }
[[community_taps]]
url = "https://example.invalid/repo.git"
future = 2026-10-08
[[external_trainer_sources]]
sourceId = "custom"
displayName = "Custom"
baseUrl = "https://example.invalid/"
sourceType = "wordpress_rss"
enabled = true
future = { enabled = true }
"#;

#[test]
fn settings_unknown_fields_survive_save_migration_and_partial_update() {
    let dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().into());
    fs::write(store.settings_path(), SETTINGS).unwrap();
    let mut loaded = store.load().unwrap();
    let original: toml::Value = toml::from_str(SETTINGS).unwrap();
    loaded.offline_mode = true;
    store.save(&loaded).unwrap();
    store
        .migrate_or_save_settings(&store.load().unwrap())
        .unwrap();
    store
        .update(|settings| {
            settings.last_used_profile = "new".into();
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    let saved: toml::Value =
        toml::from_str(&fs::read_to_string(store.settings_path()).unwrap()).unwrap();
    for key in [
        "future_float",
        "future_date",
        "ui",
        "community_taps",
        "external_trainer_sources",
        "config_history",
    ] {
        assert_eq!(saved[key], original[key], "unknown data at {key}");
    }
    assert_eq!(saved["offline_mode"].as_bool(), Some(true));
}

#[test]
fn settings_normalized_save_is_byte_stable_across_reload_and_save() {
    let dir = tempdir().unwrap();
    let store = SettingsStore::with_base_path(dir.path().into());
    fs::write(store.settings_path(), SETTINGS).unwrap();
    store.save(&store.load().unwrap()).unwrap();
    let first = fs::read(store.settings_path()).unwrap();

    store.save(&store.load().unwrap()).unwrap();

    assert_eq!(fs::read(store.settings_path()).unwrap(), first);
}

#[test]
fn settings_ipc_merge_uses_disk_extras_and_subscription_identity() {
    let current: AppSettingsData = toml::from_str(SETTINGS).unwrap();
    let mut incoming = current.clone();
    incoming.clear_extra();
    incoming.extra.insert("injected".into(), "untrusted".into());
    incoming
        .config_history
        .extra
        .insert("injected".into(), 4.into());
    incoming.community_taps[0]
        .extra
        .insert("injected".into(), true.into());
    incoming.external_trainer_sources[0]
        .extra
        .insert("injected".into(), true.into());
    incoming.offline_mode = true;
    incoming.preserve_extra_from(&current);
    assert!(incoming.offline_mode);
    assert_eq!(incoming.extra, current.extra);
    assert_eq!(incoming.config_history.extra, current.config_history.extra);
    assert_eq!(
        incoming.community_taps[0].extra,
        current.community_taps[0].extra
    );
    assert_eq!(
        incoming.external_trainer_sources[0].extra,
        current.external_trainer_sources[0].extra
    );
    let mut new = current.community_taps[0].clone();
    new.url = "https://other.invalid/".into();
    incoming.community_taps = vec![new];
    incoming.preserve_extra_from(&current);
    assert!(incoming.community_taps[0].extra.is_empty());
    incoming.clear_extra();
    assert!(!serde_json::to_string(&incoming).unwrap().contains("future"));
}

#[test]
fn recent_unknown_fields_survive_local_and_ipc_save() {
    let dir = tempdir().unwrap();
    let store = RecentFilesStore::with_path(dir.path().join("recent.toml"));
    fs::write(&store.path, "future = 2026-10-08\n[ui]\nscale = 1.5\n").unwrap();
    let loaded = store.load(10).unwrap();
    store.save(&loaded, 10).unwrap();
    let mut incoming = RecentFilesData::default();
    incoming.extra.insert("injected".into(), true.into());
    store.save_from_ipc(&incoming, 10).unwrap();
    assert_eq!(store.load(10).unwrap().extra, loaded.extra);
}

#[test]
fn malformed_known_fields_fail_instead_of_becoming_extras() {
    for content in [
        "offline_mode = 'yes'",
        "recent_files_limit = 'many'",
        "[config_history]\nmax_revisions = false",
        "community_taps = 3",
        "[ui]\nscale = 1.2\n[config_history]\nmax_revisions = []",
    ] {
        assert!(
            toml::from_str::<AppSettingsData>(content).is_err(),
            "{content}"
        );
    }
}
