use std::fs;

use crate::platform::{
    normalize_flatpak_host_path, normalized_path_exists_on_host, normalized_path_is_dir,
    normalized_path_is_dir_on_host, normalized_path_is_executable_file,
    normalized_path_is_executable_file_on_host, normalized_path_is_file_on_host,
};

pub(super) fn normalized_host_probe_path(raw_path: &str) -> String {
    normalize_flatpak_host_path(raw_path).trim().to_string()
}

pub(super) fn display_path(raw_path: &str) -> String {
    let original = raw_path.trim();
    if !original.is_empty() {
        return original.to_string();
    }
    normalized_host_probe_path(raw_path)
}

pub(super) fn path_exists_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if std::path::Path::new(original).exists() {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (std::path::Path::new(&normalized).exists()
            || normalized_path_exists_on_host(&normalized))
}

pub(super) fn path_is_file_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if std::path::Path::new(original).is_file() {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (std::path::Path::new(&normalized).is_file()
            || normalized_path_is_file_on_host(&normalized))
}

pub(super) fn path_is_dir_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if std::path::Path::new(original).is_dir() {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (normalized_path_is_dir(normalized.as_str())
            || normalized_path_is_dir_on_host(normalized.as_str()))
}

pub(super) fn path_is_executable_file(path: &str) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

pub(super) fn path_is_executable_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if path_is_executable_file(original) {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (normalized_path_is_executable_file(normalized.as_str())
            || normalized_path_is_executable_file_on_host(normalized.as_str()))
}
