mod commands;
mod paths;

use crosshook_core::profile::ProfileStore;
pub use paths::resolve_script_path;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            paths::ensure_bundled_scripts_executable(&app_handle)?;
            Ok(())
        })
        .manage(ProfileStore::new())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::export::export_launchers,
            commands::export::validate_launcher_export,
            commands::launch::launch_game,
            commands::launch::launch_trainer,
            commands::launch::validate_launch,
            commands::profile::profile_delete,
            commands::profile::profile_import_legacy,
            commands::profile::profile_list,
            commands::profile::profile_load,
            commands::profile::profile_save,
            commands::steam::auto_populate_steam,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CrossHook Native");
}
