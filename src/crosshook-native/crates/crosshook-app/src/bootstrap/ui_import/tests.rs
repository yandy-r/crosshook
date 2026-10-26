use super::*;

use std::fs;

use crosshook_core::settings::{AppSettingsData, SettingsStore};
use tempfile::tempdir;

use crate::bootstrap::{
    BootstrapOptions, BootstrapStep, FlatpakMode, LoggingMode, PreRuntime, StepLog, StepOutcome,
    StoreRoots, UmuRefresh,
};
use crate::App;

fn store_in(base: &std::path::Path) -> SettingsStore {
    fs::create_dir_all(base).unwrap();
    SettingsStore::with_base_path(base.to_path_buf())
}

fn load(store: &SettingsStore) -> AppSettingsData {
    store.load().unwrap()
}

#[test]
fn apply_import_some_list_writes_and_returns_true() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());

    assert!(apply_import(&store, Some(LibraryViewMode::List)));

    assert_eq!(
        load(&store).ui.library_view_mode,
        Some(LibraryViewMode::List)
    );
    let raw = fs::read_to_string(store.settings_path()).unwrap();
    assert!(raw.contains("library_view_mode = \"list\""), "{raw}");
}

#[test]
fn apply_import_none_writes_nothing_and_returns_false() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());

    assert!(!apply_import(&store, None));
    assert!(
        !store.settings_path().exists(),
        "None import must not create settings.toml"
    );

    // With an existing file, bytes stay identical.
    let mut settings = AppSettingsData::default();
    settings.offline_mode = true;
    store.save(&settings).unwrap();
    let before = fs::read(store.settings_path()).unwrap();

    assert!(!apply_import(&store, None));
    assert_eq!(fs::read(store.settings_path()).unwrap(), before);
}

#[test]
fn apply_import_existing_value_wins() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());
    let mut settings = AppSettingsData::default();
    settings.ui.library_view_mode = Some(LibraryViewMode::Grid);
    store.save(&settings).unwrap();

    // Simulates the key being set between the check and the update.
    assert!(!apply_import(&store, Some(LibraryViewMode::List)));
    assert_eq!(
        load(&store).ui.library_view_mode,
        Some(LibraryViewMode::Grid)
    );
}

#[test]
fn apply_import_twice_writes_once() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());

    assert!(apply_import(&store, Some(LibraryViewMode::List)));
    let before = fs::read(store.settings_path()).unwrap();
    assert!(!apply_import(&store, Some(LibraryViewMode::Grid)));
    assert_eq!(fs::read(store.settings_path()).unwrap(), before);
    assert_eq!(
        load(&store).ui.library_view_mode,
        Some(LibraryViewMode::List)
    );
}

#[test]
fn import_step_unreadable_skips() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());
    let settings = AppSettingsData::default();

    import_step(&store, &settings, true, &StoreRoots::Xdg);
    assert!(!store.settings_path().exists());
    assert_eq!(load(&store).ui.library_view_mode, None);
}

#[test]
fn import_step_explicit_roots_skips() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());
    let settings = AppSettingsData::default();
    let roots = StoreRoots::Explicit {
        config_dir: temp.path().join("config"),
        data_dir: temp.path().join("data"),
    };

    import_step(&store, &settings, false, &roots);
    assert!(!store.settings_path().exists());
    assert_eq!(load(&store).ui.library_view_mode, None);
}

#[test]
fn import_step_existing_view_mode_skips() {
    let temp = tempdir().unwrap();
    let store = store_in(temp.path());
    let mut settings = AppSettingsData::default();
    settings.ui.library_view_mode = Some(LibraryViewMode::Grid);
    store.save(&settings).unwrap();
    let before = fs::read(store.settings_path()).unwrap();

    import_step(&store, &settings, false, &StoreRoots::Xdg);
    assert_eq!(fs::read(store.settings_path()).unwrap(), before);
    assert_eq!(
        load(&store).ui.library_view_mode,
        Some(LibraryViewMode::Grid)
    );
}
#[tokio::test]
async fn bootstrap_explicit_roots_keeps_view_mode_none_and_step_list_unchanged() {
    let temp = tempdir().unwrap();
    let mut steps_log = StepLog::new();
    for step in BootstrapStep::PRE_RUNTIME_ORDER {
        steps_log.push(step, StepOutcome::Ran);
    }
    let pre = PreRuntime {
        steps: steps_log,
        mode: FlatpakMode::Native,
        migration: None,
    };
    let opts = BootstrapOptions::new()
        .with_runtime_handle(tokio::runtime::Handle::current())
        .with_store_roots(StoreRoots::Explicit {
            config_dir: temp.path().join("config"),
            data_dir: temp.path().join("data"),
        })
        .with_logging(LoggingMode::Skip)
        .with_umu_refresh(UmuRefresh::Skip {
            reason: "ui_import test",
        });

    let (app, _report) = App::bootstrap(pre, opts).unwrap();

    assert_eq!(app.services().ui_settings.get().library_view_mode, None);
    assert_eq!(
        app.stores().settings.load().unwrap().ui.library_view_mode,
        None
    );
    let steps: Vec<BootstrapStep> = app
        .bootstrap_steps()
        .records()
        .iter()
        .map(|record| record.step)
        .collect();
    assert_eq!(
        steps,
        vec![
            BootstrapStep::FlatpakGate,
            BootstrapStep::FlatpakMigration,
            BootstrapStep::LegacyAppIdMigration,
            BootstrapStep::EnvPolicy,
            BootstrapStep::Runtime,
            BootstrapStep::SettingsStore,
            BootstrapStep::Logging,
            BootstrapStep::DevelopmentScripts,
            BootstrapStep::ProfileStore,
            BootstrapStep::RecentFilesStore,
            BootstrapStep::CommunityTapStore,
            BootstrapStep::MetadataStore,
            BootstrapStep::Catalogs,
            BootstrapStep::MetadataReconciliation,
            BootstrapStep::NagSync,
            BootstrapStep::StartupJobs,
        ]
    );
}
