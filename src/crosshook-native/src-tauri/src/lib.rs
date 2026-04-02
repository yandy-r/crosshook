mod commands;
mod paths;
mod startup;

use crosshook_core::community::CommunityTapStore;
use crosshook_core::launch::{initialize_catalog, load_catalog};
use crosshook_core::logging;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::offline::{initialize_trainer_type_catalog, load_trainer_type_catalog};
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::{RecentFilesStore, SettingsStore};
pub use paths::resolve_script_path;
use tauri::{Emitter, Manager};
use tokio::time::{sleep, Duration};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Prevent GBM EGL display creation failures on multi-GPU systems (e.g. eGPU setups)
    // by disabling WebKitGTK's DMA-BUF renderer. The dev script sets this via the shell
    // environment, but the AppImage needs it set before WebKit initializes.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    let profile_store = ProfileStore::try_new().unwrap_or_else(|error| {
        eprintln!("CrossHook: failed to initialize profile store: {error}");
        std::process::exit(1);
    });
    let settings_store = SettingsStore::try_new().unwrap_or_else(|error| {
        eprintln!("CrossHook: failed to initialize settings store: {error}");
        std::process::exit(1);
    });
    let recent_files_store = RecentFilesStore::try_new().unwrap_or_else(|error| {
        eprintln!("CrossHook: failed to initialize recent files store: {error}");
        std::process::exit(1);
    });
    let community_tap_store = CommunityTapStore::try_new().unwrap_or_else(|error| {
        eprintln!("CrossHook: failed to initialize community tap store: {error}");
        std::process::exit(1);
    });
    let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
        tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
        MetadataStore::disabled()
    });
    let metadata_for_startup = metadata_store.clone();

    // Initialize the optimization catalog before any command handler runs.
    // Merge order: embedded default → user override (~/.config/crosshook/optimization_catalog.toml).
    let catalog = load_catalog(Some(&settings_store.base_path), &[]);
    initialize_catalog(catalog);

    let trainer_type_catalog = load_trainer_type_catalog(Some(&settings_store.base_path), &[]);
    initialize_trainer_type_catalog(trainer_type_catalog);

    tauri::Builder::default()
        .setup({
            let profile_store = profile_store.clone();
            let settings_store = settings_store.clone();
            let metadata_for_startup = metadata_for_startup.clone();

            move |app| {
                let log_path = logging::init_logging(false)?;
                tracing::info!(log_path = %log_path.display(), "starting CrossHook Native");

                paths::ensure_development_scripts_executable()?;

                let auto_load_profile_name =
                    startup::resolve_auto_load_profile_name(&settings_store, &profile_store)?;

                if let Err(error) =
                    startup::run_metadata_reconciliation(&metadata_for_startup, &profile_store)
                {
                    tracing::warn!(%error, "startup metadata reconciliation failed");
                }

                {
                    let catalog = crosshook_core::launch::global_catalog();
                    if let Err(error) = metadata_for_startup.persist_optimization_catalog(
                        &catalog.entries,
                        catalog.catalog_version,
                    ) {
                        tracing::warn!(%error, "failed to persist optimization catalog to metadata db");
                    }
                }

                if let Some(profile_name) = auto_load_profile_name {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        sleep(Duration::from_millis(350)).await;
                        if let Err(error) = app_handle.emit("auto-load-profile", &profile_name) {
                            tracing::warn!(
                                %error,
                                profile_name,
                                "failed to emit auto-load-profile event"
                            );
                        }
                    });
                }

                {
                    let show_onboarding = settings_store
                        .load()
                        .map(|s| !s.onboarding_completed)
                        .unwrap_or(true);
                    let has_profiles = profile_store
                        .list()
                        .map(|p| !p.is_empty())
                        .unwrap_or(false);
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        sleep(Duration::from_millis(350)).await;

                        #[derive(serde::Serialize)]
                        struct OnboardingCheckPayload {
                            show: bool,
                            has_profiles: bool,
                        }

                        if let Err(error) = app_handle.emit(
                            "onboarding-check",
                            &OnboardingCheckPayload {
                                show: show_onboarding,
                                has_profiles,
                            },
                        ) {
                            tracing::warn!(%error, "failed to emit onboarding-check event");
                        }
                    });
                }

                {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        sleep(Duration::from_millis(500)).await;
                        let store = app_handle.state::<ProfileStore>();
                        let metadata_store = app_handle.state::<MetadataStore>();
                        let summary = commands::health::build_enriched_health_summary(
                            &store,
                            &metadata_store,
                        );
                        match app_handle.emit("profile-health-batch-complete", &summary) {
                            Ok(()) => {
                                tracing::info!(
                                    total = summary.total_count,
                                    healthy = summary.healthy_count,
                                    stale = summary.stale_count,
                                    broken = summary.broken_count,
                                    "startup health scan complete"
                                );
                            }
                            Err(error) => {
                                tracing::warn!(
                                    %error,
                                    "failed to emit profile-health-batch-complete event"
                                );
                            }
                        }
                        #[derive(serde::Serialize)]
                        struct OfflineReadinessScanComplete {
                            total_profiles: usize,
                        }
                        if let Err(error) = app_handle.emit(
                            "offline-readiness-scan-complete",
                            &OfflineReadinessScanComplete {
                                total_profiles: summary.total_count,
                            },
                        ) {
                            tracing::warn!(
                                %error,
                                "failed to emit offline-readiness-scan-complete event"
                            );
                        }
                    });
                }

                {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        sleep(Duration::from_millis(2000)).await;
                        startup::run_version_scan(app_handle).await;
                    });
                }

                Ok(())
            }
        })
        .manage(profile_store)
        .manage(settings_store)
        .manage(recent_files_store)
        .manage(community_tap_store)
        .manage(metadata_store)
        .manage(commands::update::UpdateProcessState::new())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::export::export_launchers,
            commands::export::validate_launcher_export,
            commands::export::check_launcher_exists,
            commands::export::check_launcher_for_profile,
            commands::export::delete_launcher,
            commands::export::delete_launcher_by_slug,
            commands::export::rename_launcher,
            commands::export::list_launchers,
            commands::export::reexport_launcher_by_slug,
            commands::export::find_orphaned_launchers,
            commands::export::preview_launcher_script,
            commands::export::preview_launcher_desktop,
            commands::community::community_add_tap,
            commands::community::community_export_profile,
            commands::community::community_import_profile,
            commands::community::community_prepare_import,
            commands::community::community_list_profiles,
            commands::community::community_sync,
            commands::install::install_default_prefix_path,
            commands::install::install_game,
            commands::install::validate_install_request,
            commands::launch::launch_game,
            commands::launch::launch_trainer,
            commands::launch::validate_launch,
            commands::launch::preview_launch,
            commands::launch::build_steam_launch_options_command,
            commands::launch::check_gamescope_session,
            commands::profile::profile_delete,
            commands::profile::profile_duplicate,
            commands::profile::profile_import_legacy,
            commands::profile::profile_list,
            commands::profile::profile_list_summaries,
            commands::profile::profile_load,
            commands::profile::profile_rename,
            commands::profile::profile_save,
            commands::profile::profile_save_launch_optimizations,
            commands::profile::profile_save_mangohud_config,
            commands::profile::profile_list_bundled_optimization_presets,
            commands::profile::profile_apply_bundled_optimization_preset,
            commands::profile::profile_save_manual_optimization_preset,
            commands::profile::profile_export_toml,
            commands::protondb::protondb_lookup,
            commands::game_metadata::fetch_game_metadata,
            commands::game_metadata::fetch_game_cover_art,
            commands::game_metadata::import_custom_cover_art,
            commands::settings::recent_files_load,
            commands::settings::recent_files_save,
            commands::settings::settings_load,
            commands::settings::settings_save,
            commands::steam::auto_populate_steam,
            commands::steam::default_steam_client_install_path,
            commands::steam::list_proton_installs,
            commands::update::validate_update_request,
            commands::update::update_game,
            commands::update::cancel_update,
            // Phase 3: Catalog and Intelligence
            commands::community::community_list_indexed_profiles,
            commands::collections::collection_list,
            commands::collections::collection_create,
            commands::collections::collection_delete,
            commands::collections::collection_add_profile,
            commands::collections::collection_remove_profile,
            commands::collections::collection_list_profiles,
            commands::profile::profile_set_favorite,
            commands::profile::profile_list_favorites,
            commands::profile::profile_config_history,
            commands::profile::profile_config_diff,
            commands::profile::profile_config_rollback,
            commands::profile::profile_mark_known_good,
            commands::health::batch_validate_profiles,
            commands::health::get_profile_health,
            commands::health::get_cached_health_snapshots,
            commands::health::get_cached_offline_readiness_snapshots,
            commands::diagnostics::export_diagnostics,
            commands::migration::check_proton_migrations,
            commands::migration::apply_proton_migration,
            commands::migration::apply_batch_migration,
            commands::version::check_version_status,
            commands::version::get_version_snapshot,
            commands::version::set_trainer_version,
            commands::version::acknowledge_version_change,
            commands::onboarding::check_readiness,
            commands::onboarding::dismiss_onboarding,
            commands::onboarding::get_trainer_guidance,
            commands::catalog::get_optimization_catalog,
            commands::catalog::get_mangohud_presets,
            commands::offline::check_offline_readiness,
            commands::offline::batch_offline_readiness,
            commands::offline::verify_trainer_hash,
            commands::offline::check_network_status,
            commands::offline::get_trainer_type_catalog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CrossHook Native");
}
