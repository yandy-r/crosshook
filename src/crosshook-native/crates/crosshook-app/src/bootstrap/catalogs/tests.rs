use super::*;

use crosshook_core::settings::SettingsStore;
use tempfile::{tempdir, TempDir};

/// Settings store rooted under a live tempdir guard. Catalog globals own
/// their parsed contents (no path references), but the config directory must
/// exist while `initialize` reads it, so callers keep the guard until the end
/// of the test.
fn settings_store() -> (TempDir, SettingsStore) {
    let temp = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp.path().join("config"));
    (temp, store)
}

#[test]
fn initialize_installs_global_catalogs() {
    let (_guard, settings) = settings_store();

    initialize(&settings);

    // OnceLock globals are now set; accessors must not panic and must expose
    // the embedded defaults.
    let catalog = crosshook_core::launch::global_catalog();
    assert!(!catalog.entries.is_empty());
    let readiness = crosshook_core::onboarding::global_readiness_catalog();
    assert!(!readiness.entries.is_empty());
    let _ = crosshook_core::onboarding::global_capability_map();
    let _ = crosshook_core::offline::global_trainer_type_catalog();
    let _ = crosshook_core::launch::global_command_argument_catalog();
}

#[test]
fn initialize_is_idempotent_across_repeated_calls() {
    let (_guard, settings) = settings_store();

    initialize(&settings);
    let first = crosshook_core::launch::global_catalog().catalog_version;
    initialize(&settings);
    let second = crosshook_core::launch::global_catalog().catalog_version;

    assert_eq!(first, second);
}

#[test]
fn persist_is_noop_on_disabled_metadata() {
    let metadata = MetadataStore::disabled();

    // Must not panic; logs a skip warning.
    persist(&metadata);
}

#[test]
fn persist_writes_catalogs_to_healthy_metadata() {
    let (_guard, settings) = settings_store();
    initialize(&settings);
    let metadata = MetadataStore::open_in_memory().unwrap();

    persist(&metadata);

    assert_eq!(metadata.status(), MetadataStatus::Ok);
}
