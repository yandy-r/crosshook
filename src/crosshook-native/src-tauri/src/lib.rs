mod commands;
mod env_policy;
mod fatal;
mod startup_bridge;

pub use crosshook_app::services::{BackgroundGrantHolder, BackgroundProtectionState};

use crosshook_app::bootstrap::{pre_runtime, BootstrapOptions, PreRuntimeOptions, UmuRefresh};
use crosshook_app::App;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let pre =
        match pre_runtime(PreRuntimeOptions::new().with_env_policy(env_policy::WebKitEnvPolicy)) {
            Ok(pre) => pre,
            Err(error) => fatal::show_and_exit(context, error),
        };
    let options = BootstrapOptions::new()
        .with_runtime_handle(tauri::async_runtime::handle().inner().clone())
        .with_umu_refresh(if commands::bench::bench_enabled() {
            UmuRefresh::Skip {
                reason: "CROSSHOOK_BENCH=1",
            }
        } else {
            UmuRefresh::Enabled
        });
    let (core, _) = match App::bootstrap(pre, options) {
        Ok(result) => result,
        Err(error) => fatal::show_and_exit(context, error),
    };
    let startup_events = core.subscribe();
    let stores = core.stores();
    let services = core.services();
    let ui_settings = services.ui_settings.clone();
    tauri::Builder::default()
        .manage(stores.settings.clone())
        .manage(stores.profiles.clone())
        .manage(stores.recent_files.clone())
        .manage(stores.community_taps.clone())
        .manage(stores.metadata.clone())
        .manage(services.settings.clone())
        .manage(services.ui_settings.clone())
        .manage(services.recent_files.clone())
        .manage(services.diagnostics.clone())
        .manage(services.health.clone())
        .manage(services.profiles.clone())
        .manage(services.background.clone())
        .manage(services.launch.clone())
        .manage(services.storage.clone())
        .manage(services.mods.clone())
        .manage(services.collections.clone())
        .manage(services.launcher_export.clone())
        .manage(services.version.clone())
        .manage(services.steam.clone())
        .manage(core.clone())
        .setup(move |app| {
            startup_bridge::spawn(app.handle().clone(), core.startup(), startup_events);
            Ok(())
        })
        .manage(commands::update::UpdateProcessState::new())
        .manage(commands::run_executable::RunExecutableProcessState::new())
        .manage(commands::prefix_deps::PrefixDepsInstallState::new())
        .manage(std::sync::Arc::new(
            commands::protonup::ProtonInstallRegistry::default(),
        ))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::bench::bench_ready,
            commands::startup::get_startup_report,
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
            commands::lutris::lutris_prepare_import,
            commands::lutris::lutris_import_profiles,
            commands::community::community_list_profiles,
            commands::community::community_sync,
            commands::shared::normalize_host_path,
            commands::install::install_default_prefix_path,
            commands::install::install_game,
            commands::install::validate_install_request,
            commands::launch::launch_game,
            commands::launch::launch_trainer,
            commands::launch::launch_reset_sessions,
            commands::launch::validate_launch,
            commands::launch::preview_launch,
            commands::launch::build_steam_launch_options_command,
            commands::launch::check_gamescope_session,
            commands::launch::launch_platform_status,
            commands::launch::check_game_running,
            commands::launch::list_running_profiles,
            commands::launch::list_launch_history_for_profile,
            commands::mods::list_profile_mods,
            commands::mods::add_profile_mod,
            commands::mods::update_profile_mod,
            commands::mods::remove_profile_mod,
            commands::mods::detect_profile_mods,
            commands::mods::analyze_mod_coexistence,
            commands::profile::profile_delete,
            commands::profile::profile_duplicate,
            commands::profile::profile_import_legacy,
            commands::profile::profile_list,
            commands::profile::profile_list_summaries,
            commands::profile::profile_load,
            commands::profile::profile_rename,
            commands::profile::profile_save,
            commands::profile::profile_save_launch_optimizations,
            commands::profile::profile_save_command_arguments,
            commands::profile::profile_save_mangohud_config,
            commands::profile::profile_save_gamescope_config,
            commands::profile::profile_save_trainer_gamescope_config,
            commands::profile::profile_list_bundled_optimization_presets,
            commands::profile::profile_apply_bundled_optimization_preset,
            commands::profile::profile_save_manual_optimization_preset,
            commands::profile::profile_export_toml,
            commands::protondb::protondb_lookup,
            commands::protondb::protondb_get_suggestions,
            commands::protondb::protondb_accept_suggestion,
            commands::protondb::protondb_dismiss_suggestion,
            commands::game_metadata::fetch_game_metadata,
            commands::game_metadata::fetch_game_cover_art,
            commands::game_metadata::import_custom_cover_art,
            commands::game_metadata::import_custom_art,
            commands::settings::recent_files_load,
            commands::settings::recent_files_save,
            commands::settings::settings_load,
            commands::settings::settings_save,
            commands::settings::settings_save_steamgriddb_key,
            commands::settings::settings_set_library_view_mode,
            commands::storage::scan_prefix_storage,
            commands::storage::cleanup_prefix_storage,
            commands::storage::get_prefix_storage_history,
            commands::steam::auto_populate_steam,
            commands::steam::default_steam_client_install_path,
            commands::steam::list_proton_installs,
            commands::update::validate_update_request,
            commands::update::update_game,
            commands::update::cancel_update,
            commands::run_executable::validate_run_executable_request,
            commands::run_executable::run_executable,
            commands::run_executable::cancel_run_executable,
            commands::run_executable::stop_run_executable,
            // Phase 3: Catalog and Intelligence
            commands::community::community_list_indexed_profiles,
            commands::collections::collection_list,
            commands::collections::collection_create,
            commands::collections::collection_delete,
            commands::collections::collection_add_profile,
            commands::collections::collection_remove_profile,
            commands::collections::collection_list_profiles,
            commands::collections::collection_rename,
            commands::collections::collection_update_description,
            commands::collections::collections_for_profile,
            commands::collections::collection_get_defaults,
            commands::collections::collection_set_defaults,
            commands::collections::collection_export_to_toml,
            commands::collections::collection_import_from_toml,
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
            commands::metadata::metadata_store_status,
            commands::migration::check_proton_migrations,
            commands::migration::apply_proton_migration,
            commands::migration::apply_batch_migration,
            commands::version::check_version_status,
            commands::version::get_version_snapshot,
            commands::version::set_trainer_version,
            commands::version::acknowledge_version_change,
            commands::onboarding::check_readiness,
            commands::onboarding::check_generalized_readiness,
            commands::onboarding::probe_host_tool_details,
            commands::onboarding::get_cached_host_readiness_snapshot,
            commands::onboarding::get_capabilities,
            commands::background::get_background_protection_state,
            commands::onboarding::dismiss_onboarding,
            commands::onboarding::dismiss_umu_install_nag,
            commands::onboarding::dismiss_steam_deck_caveats,
            commands::onboarding::dismiss_readiness_nag,
            commands::onboarding::get_trainer_guidance,
            commands::catalog::get_optimization_catalog,
            commands::catalog::get_command_argument_catalog,
            commands::catalog::get_mangohud_presets,
            commands::offline::check_offline_readiness,
            commands::offline::batch_offline_readiness,
            commands::offline::verify_trainer_hash,
            commands::offline::check_network_status,
            commands::offline::get_trainer_type_catalog,
            // Prefix dependency management
            commands::prefix_deps::detect_protontricks_binary,
            commands::prefix_deps::check_prefix_dependencies,
            commands::prefix_deps::install_prefix_dependency,
            commands::prefix_deps::get_dependency_status,
            commands::prefix_deps::get_prefix_version_repair_status,
            commands::prefix_deps::repair_prefix_windows_version,
            // Trainer discovery
            commands::discovery::discovery_catalog,
            commands::discovery::discovery_search_external,
            commands::discovery::discovery_check_version_compatibility,
            commands::discovery::discovery_list_external_sources,
            commands::discovery::discovery_add_external_source,
            commands::discovery::discovery_remove_external_source,
            // ProtonUp integration
            commands::protonup::protonup_list_available_versions,
            commands::protonup::protonup_install_version,
            commands::protonup::protonup_get_suggestion,
            commands::protonup::protonup_list_providers,
            commands::protonup::protonup_resolve_install_roots,
            commands::protonup::protonup_install_version_async,
            commands::protonup::protonup_cancel_install,
            commands::protonup::protonup_plan_uninstall_version,
            commands::protonup::protonup_uninstall_version,
            // UMU database
            commands::umu_database::refresh_umu_database,
            commands::umu_database::check_umu_coverage,
            commands::umu_database::clear_umu_gameid_lookup_cache,
        ])
        .build(context)
        .expect("error while building CrossHook Native")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Persist any trailing `[ui]` write still waiting on the throttle.
                if let Err(error) = tauri::async_runtime::block_on(ui_settings.flush()) {
                    tracing::warn!(error = %error.message(), "failed to flush [ui] settings on exit");
                }
            }
        });
}
