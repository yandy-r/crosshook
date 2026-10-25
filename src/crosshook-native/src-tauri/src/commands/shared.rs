#[tauri::command]
pub fn normalize_host_path(path: String) -> String {
    crosshook_core::platform::normalize_flatpak_host_path(&path)
}

pub fn sanitize_display_path(path: &str) -> String {
    crosshook_app::presenter::display_path_tilde(path)
}
