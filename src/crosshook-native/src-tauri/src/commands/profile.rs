use crosshook_core::profile::{GameProfile, ProfileStore, ProfileStoreError};
use tauri::State;

fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}

#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}

#[tauri::command]
pub fn profile_load(name: String, store: State<'_, ProfileStore>) -> Result<GameProfile, String> {
    store.load(&name).map_err(map_error)
}

#[tauri::command]
pub fn profile_save(
    name: String,
    data: GameProfile,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.save(&name, &data).map_err(map_error)
}

#[tauri::command]
pub fn profile_delete(name: String, store: State<'_, ProfileStore>) -> Result<(), String> {
    // Best-effort launcher cleanup before profile deletion.
    // Profile deletion must succeed even if launcher cleanup fails.
    if let Ok(profile) = store.load(&name) {
        if profile.launch.method != "native" {
            if let Err(e) =
                crosshook_core::export::delete_launcher_for_profile(&profile, "", "")
            {
                tracing::warn!("Launcher cleanup failed for profile {name}: {e}");
            }
        }
    }

    store.delete(&name).map_err(map_error)
}

#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}

#[tauri::command]
pub fn profile_import_legacy(
    path: String,
    store: State<'_, ProfileStore>,
) -> Result<GameProfile, String> {
    store
        .import_legacy(std::path::Path::new(&path))
        .map_err(map_error)
}
