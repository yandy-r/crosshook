use std::env;
use std::path::Path;

#[tauri::command]
pub fn normalize_host_path(path: String) -> String {
    crosshook_core::platform::normalize_flatpak_host_path(&path)
}

pub fn sanitize_display_path(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) => {
            let path = Path::new(path);
            let home = Path::new(&home);
            match path.strip_prefix(home) {
                Ok(suffix) if suffix.as_os_str().is_empty() => "~/".to_string(),
                Ok(suffix) => format!("~/{}", suffix.display()),
                Err(_) => path.display().to_string(),
            }
        }
        _ => path.to_string(),
    }
}
