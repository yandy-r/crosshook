use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::platform::normalize_flatpak_host_path;

pub(crate) fn combine_host_unix_path(
    root_path: &str,
    segment_one: &str,
    segment_two: &str,
) -> String {
    let normalized_root_path = normalize_host_unix_path(root_path);
    let normalized_root_path = normalized_root_path.trim_end_matches('/');
    if normalized_root_path.is_empty() {
        return String::new();
    }

    let mut result = normalized_root_path.to_string();
    for segment in [segment_one, segment_two] {
        let normalized_segment = normalize_host_unix_path(segment);
        let normalized_segment = normalized_segment.trim_matches('/');
        if normalized_segment.is_empty() {
            continue;
        }

        result.push('/');
        result.push_str(normalized_segment);
    }

    result
}

pub(crate) fn write_host_text_file(
    host_path: &str,
    content: &str,
    mode: u32,
) -> Result<(), io::Error> {
    let writable_path = PathBuf::from(host_path);
    let directory_path = writable_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("could not resolve a parent directory for '{host_path}'"),
        )
    })?;

    fs::create_dir_all(directory_path)?;
    fs::write(&writable_path, content.replace("\r\n", "\n"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&writable_path)?.permissions();
        permissions.set_mode(mode);
        fs::set_permissions(&writable_path, permissions)?;
    }

    Ok(())
}

pub fn resolve_target_home_path(
    preferred_home_path: &str,
    steam_client_install_path: &str,
) -> String {
    let normalized_preferred_home_path = normalize_host_unix_path(preferred_home_path);
    if looks_like_usable_host_unix_path(&normalized_preferred_home_path) {
        return normalized_preferred_home_path;
    }

    let normalized_steam_client_install_path = normalize_host_unix_path(steam_client_install_path);
    if let Some(derived_home_path) =
        try_resolve_home_from_steam_client_install_path(&normalized_steam_client_install_path)
    {
        return derived_home_path;
    }

    if let Ok(home_path) = env::var("HOME") {
        let normalized_home_path = normalize_host_unix_path(&home_path);
        if looks_like_usable_host_unix_path(&normalized_home_path) {
            return normalized_home_path;
        }
    }

    if looks_like_usable_host_unix_path(&normalized_preferred_home_path) {
        normalized_preferred_home_path
    } else {
        String::new()
    }
}

fn try_resolve_home_from_steam_client_install_path(
    steam_client_install_path: &str,
) -> Option<String> {
    const LOCAL_SHARE_STEAM_SUFFIX: &str = "/.local/share/Steam";
    const DOT_STEAM_ROOT_SUFFIX: &str = "/.steam/root";
    const FLATPAK_STEAM_SUFFIX: &str = "/.var/app/com.valvesoftware.Steam/data/Steam";

    if steam_client_install_path.trim().is_empty() {
        return None;
    }

    if let Some(home_path) = steam_client_install_path.strip_suffix(LOCAL_SHARE_STEAM_SUFFIX) {
        let home_path = home_path.trim();
        if !home_path.is_empty() {
            return Some(home_path.to_string());
        }
    }

    if let Some(home_path) = steam_client_install_path.strip_suffix(DOT_STEAM_ROOT_SUFFIX) {
        let home_path = home_path.trim();
        if !home_path.is_empty() {
            return Some(home_path.to_string());
        }
    }

    if let Some(home_path) = steam_client_install_path.strip_suffix(FLATPAK_STEAM_SUFFIX) {
        let home_path = home_path.trim();
        if !home_path.is_empty() {
            return Some(home_path.to_string());
        }
    }

    None
}

fn resolve_desktop_icon_value(launcher_icon_path: &str) -> String {
    let normalized_launcher_icon_path = normalize_host_unix_path(launcher_icon_path);
    if normalized_launcher_icon_path.trim().is_empty() {
        "applications-games".to_string()
    } else {
        normalized_launcher_icon_path
    }
}

pub(crate) fn normalize_host_unix_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    normalize_flatpak_host_path(&normalized)
}

fn looks_like_usable_host_unix_path(path: &str) -> bool {
    !path.trim().is_empty() && path.starts_with('/') && !path.contains("/compatdata/")
}

pub(crate) fn shell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(crate) fn escape_desktop_exec_argument(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "%%")
        .replace(' ', "\\ ")
        .replace('"', "\\\"")
}

pub(crate) fn desktop_icon_value(launcher_icon_path: &str) -> String {
    resolve_desktop_icon_value(launcher_icon_path)
}
