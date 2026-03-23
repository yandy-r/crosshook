mod commands;
mod paths;
mod startup;

use crosshook_core::community::CommunityTapStore;
use crosshook_core::logging;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::{RecentFilesStore, SettingsStore};
pub use paths::resolve_script_path;
use tauri::Emitter;
use tokio::time::{sleep, Duration};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let profile_store = ProfileStore::new();
    let settings_store = SettingsStore::new();
    let recent_files_store = RecentFilesStore::new();
    let community_tap_store = CommunityTapStore::new();

    tauri::Builder::default()
        .setup({
            let profile_store = profile_store.clone();
            let settings_store = settings_store.clone();

            move |app| {
                let log_path = logging::init_logging(false)?;
                tracing::info!(log_path = %log_path.display(), "starting CrossHook Native");

                paths::ensure_development_scripts_executable()?;

                if let Some(profile_name) =
                    startup::resolve_auto_load_profile_name(&settings_store, &profile_store)?
                {
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

                Ok(())
            }
        })
        .manage(profile_store)
        .manage(settings_store)
        .manage(recent_files_store)
        .manage(community_tap_store)
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::export::export_launchers,
            commands::export::validate_launcher_export,
            commands::community::community_add_tap,
            commands::community::community_import_profile,
            commands::community::community_list_profiles,
            commands::community::community_sync,
            commands::launch::launch_game,
            commands::launch::launch_trainer,
            commands::launch::validate_launch,
            commands::profile::profile_delete,
            commands::profile::profile_import_legacy,
            commands::profile::profile_list,
            commands::profile::profile_load,
            commands::profile::profile_save,
            commands::settings::recent_files_load,
            commands::settings::recent_files_save,
            commands::settings::settings_load,
            commands::settings::settings_save,
            commands::steam::auto_populate_steam,
            commands::steam::default_steam_client_install_path,
            commands::steam::list_proton_installs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CrossHook Native");
}
