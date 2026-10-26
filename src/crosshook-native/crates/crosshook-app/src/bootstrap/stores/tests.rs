use super::*;

use std::fs;

use crosshook_core::settings::AppSettingsData;
use tempfile::tempdir;

fn explicit_roots(config: &std::path::Path, data: &std::path::Path) -> StoreRoots {
    StoreRoots::Explicit {
        config_dir: config.to_path_buf(),
        data_dir: data.to_path_buf(),
    }
}

#[test]
fn init_settings_explicit_creates_config_dir() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("config");
    let roots = explicit_roots(&config, &temp.path().join("data"));

    let store = init_settings(&roots).unwrap();

    assert_eq!(store.base_path, config);
    assert!(config.is_dir());
}

#[test]
fn init_settings_explicit_file_as_root_is_store_init() {
    let temp = tempdir().unwrap();
    let file_root = temp.path().join("a-file");
    fs::write(&file_root, b"not a directory").unwrap();
    let roots = explicit_roots(&file_root, &temp.path().join("data"));

    let error = init_settings(&roots).unwrap_err();

    match error {
        BootstrapError::StoreInit { store, detail } => {
            assert_eq!(store, StoreKind::Settings);
            assert!(
                detail.contains("configuration directory"),
                "unexpected detail: {detail}"
            );
        }
        other => panic!("expected StoreInit, got {other:?}"),
    }
}

#[test]
fn init_remaining_explicit_uses_data_dir_without_xdg() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("config");
    let data = temp.path().join("data");
    let roots = explicit_roots(&config, &data);
    let settings = init_settings(&roots).unwrap();

    let (stores, metadata_error) =
        init_remaining(&roots, &settings, &AppSettingsData::default()).unwrap();

    assert_eq!(metadata_error, None);
    assert_eq!(
        stores.profiles.base_path,
        config.join("profiles"),
        "profiles must default to <config>/profiles"
    );
    assert_eq!(stores.recent_files.path, data.join("recent.toml"));
    assert!(stores.metadata.is_available());
    // Community taps root lives under the explicit data dir (no XDG fallback).
    let taps_dir = data.join("community").join("taps");
    fs::create_dir_all(&taps_dir).unwrap();
    assert!(taps_dir.is_dir());
}

#[test]
fn init_remaining_explicit_directory_as_metadata_db_degrades_not_fatal() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("config");
    let data = temp.path().join("data");
    fs::create_dir_all(&data).unwrap();
    // A directory where metadata.db must be a database file: opening it fails
    // deterministically (no corruption auto-recovery path applies), so the
    // store degrades instead of failing startup.
    fs::create_dir_all(data.join("metadata.db")).unwrap();
    let roots = explicit_roots(&config, &data);
    let settings = init_settings(&roots).unwrap();

    let (stores, metadata_error) =
        init_remaining(&roots, &settings, &AppSettingsData::default()).unwrap();

    assert!(!stores.metadata.is_available());
    let detail = metadata_error.expect("degraded metadata must carry a detail");
    assert!(!detail.is_empty());
}

#[test]
fn init_remaining_explicit_file_as_data_dir_is_fatal_for_recent_files() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("config");
    let file_root = temp.path().join("data-file");
    fs::write(&file_root, b"not a directory").unwrap();
    let roots = explicit_roots(&config, &file_root);
    let settings = init_settings(&roots).unwrap();

    let error = init_remaining(&roots, &settings, &AppSettingsData::default())
        .err()
        .expect("file-as-data-root must fail store initialization");

    match error {
        BootstrapError::StoreInit { store, .. } => {
            assert!(
                matches!(store, StoreKind::RecentFiles | StoreKind::Profiles),
                "expected a fatal store kind, got {store}"
            );
        }
        other => panic!("expected StoreInit, got {other:?}"),
    }
}

#[tokio::test]
async fn services_new_builds_bundle_from_stores() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("config");
    let data = temp.path().join("data");
    let roots = explicit_roots(&config, &data);
    let settings = init_settings(&roots).unwrap();
    let (stores, _) = init_remaining(&roots, &settings, &AppSettingsData::default()).unwrap();

    let services = Services::new(
        &stores,
        crate::EventBus::default(),
        tokio::runtime::Handle::current(),
    );

    // Bundle is built; clones are cheap and Arc-shared background holder.
    let cloned = services.clone();
    assert!(Arc::ptr_eq(&services.background, &cloned.background));
}
