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
    let jobs = crate::jobs::JobRegistry::default();

    let services = Services::new(
        &stores,
        crate::EventBus::default(),
        tokio::runtime::Handle::current(),
        jobs.clone(),
    );

    // Bundle is built; clones are cheap and Arc-shared background holder.
    let cloned = services.clone();
    assert!(Arc::ptr_eq(&services.background, &cloned.background));

    // The storage service owns a clone of the app job registry, so the exclusive
    // scan key held through `jobs` is a conflict for the service's registry.
    let spec = || {
        crate::jobs::JobSpec::new(crate::services::storage::STORAGE_SCAN_JOB_KIND).with_key(
            crate::jobs::JobKey::new(crate::services::storage::STORAGE_SCAN_JOB_KEY),
        )
    };
    let held = jobs
        .start(spec(), |ctx| async move {
            ctx.cancelled().await;
            crate::jobs::Outcome::Cancelled
        })
        .unwrap();
    let conflict = services
        .storage
        .jobs()
        .start(spec(), |_ctx| async { crate::jobs::Outcome::Succeeded })
        .err()
        .expect("the storage service must share the app job registry");
    assert_eq!(conflict.kind(), crate::AppErrorKind::Conflict);
    let held_id = held.id().clone();
    let mut held_events = held.into_events();
    jobs.cancel(&held_id).unwrap();
    while let Some(event) = held_events.recv().await {
        if matches!(event.kind, crate::jobs::JobEventKind::Finished(_)) {
            break;
        }
    }
    jobs.reap(&held_id).unwrap();

    // The protonup service shares the app registry: an install key held
    // through `jobs` conflicts for the service's registry.
    let proton_spec = || {
        crate::jobs::JobSpec::new(crate::services::protonup::PROTONUP_INSTALL_JOB_KIND)
            .with_key(crate::jobs::JobKey::new("protonup-install:x:y"))
    };
    let proton_held = jobs
        .start(proton_spec(), |ctx| async move {
            ctx.cancelled().await;
            crate::jobs::Outcome::Cancelled
        })
        .unwrap();
    let proton_conflict = services
        .protonup
        .jobs()
        .start(proton_spec(), |_ctx| async {
            crate::jobs::Outcome::Succeeded
        })
        .err()
        .expect("the protonup service must share the app job registry");
    assert_eq!(proton_conflict.kind(), crate::AppErrorKind::Conflict);
    let proton_held_id = proton_held.id().clone();
    let mut proton_events = proton_held.into_events();
    jobs.cancel(&proton_held_id).unwrap();
    while let Some(event) = proton_events.recv().await {
        if matches!(event.kind, crate::jobs::JobEventKind::Finished(_)) {
            break;
        }
    }
    jobs.reap(&proton_held_id).unwrap();

    // The four process services share the app registry: a prefix key held
    // through `prefix_deps` conflicts for every other process service.
    let prefix_key = crate::services::process::prefix_job_key_for_path(std::path::Path::new(
        "/wiring-test/prefix/pfx",
    ));
    let prefix_spec =
        |kind: &'static str| crate::jobs::JobSpec::new(kind).with_key(prefix_key.clone());
    let prefix_held = services
        .prefix_deps
        .jobs()
        .start(
            prefix_spec(crate::services::prefix_deps::PREFIX_DEPS_INSTALL_JOB_KIND),
            |ctx| async move {
                ctx.cancelled().await;
                crate::jobs::Outcome::Cancelled
            },
        )
        .unwrap();
    assert!(jobs.is_key_held(&prefix_key));
    let process_registries = [
        (
            crate::services::update::UPDATE_GAME_JOB_KIND,
            services.update.jobs(),
        ),
        (
            crate::services::install::INSTALL_GAME_JOB_KIND,
            services.install.jobs(),
        ),
        (
            crate::services::run_executable::RUN_EXECUTABLE_JOB_KIND,
            services.run_executable.jobs(),
        ),
    ];
    for (kind, registry) in process_registries {
        let conflict = registry
            .start(prefix_spec(kind), |_ctx| async {
                crate::jobs::Outcome::Succeeded
            })
            .err()
            .unwrap_or_else(|| panic!("the {kind} service must share the app job registry"));
        assert_eq!(conflict.kind(), crate::AppErrorKind::Conflict);
    }
    let prefix_held_id = prefix_held.id().clone();
    let mut prefix_events = prefix_held.into_events();
    jobs.cancel(&prefix_held_id).unwrap();
    while let Some(event) = prefix_events.recv().await {
        if matches!(event.kind, crate::jobs::JobEventKind::Finished(_)) {
            break;
        }
    }
    jobs.reap(&prefix_held_id).unwrap();
    assert!(!jobs.is_key_held(&prefix_key));

    // The community service starts sync jobs on the same app registry.
    let observed = services
        .community
        .jobs()
        .start(
            crate::jobs::JobSpec::new(crate::services::community::COMMUNITY_SYNC_JOB_KIND),
            |ctx| async move {
                ctx.cancelled().await;
                crate::jobs::Outcome::Cancelled
            },
        )
        .unwrap();
    assert_eq!(jobs.job_counts(), (1, 0));
    observed.cancel();
}
