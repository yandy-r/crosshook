#[tauri::command]
pub fn normalize_host_path(path: String) -> String {
    crosshook_core::platform::normalize_flatpak_host_path(&path)
}
