use std::fs;
use std::path::Path;

use crate::platform::{
    normalize_flatpak_host_path, normalized_path_exists_on_host, normalized_path_is_dir,
    normalized_path_is_dir_on_host, normalized_path_is_executable_file,
    normalized_path_is_executable_file_on_host, normalized_path_is_file_on_host,
};

use super::error::ValidationError;

fn normalized_host_probe_path(raw_path: &str) -> String {
    normalize_flatpak_host_path(raw_path).trim().to_string()
}

pub(crate) fn path_exists_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if Path::new(original).exists() {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (Path::new(&normalized).exists() || normalized_path_exists_on_host(&normalized))
}

pub(super) fn path_is_file_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if Path::new(original).is_file() {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (Path::new(&normalized).is_file() || normalized_path_is_file_on_host(&normalized))
}

pub(super) fn path_is_dir_visible_or_host(raw_path: &str) -> bool {
    let original = raw_path.trim();
    if original.is_empty() {
        return false;
    }

    if Path::new(original).is_dir() {
        return true;
    }

    let normalized = normalized_host_probe_path(raw_path);
    !normalized.is_empty()
        && (normalized_path_is_dir(normalized.as_str())
            || normalized_path_is_dir_on_host(normalized.as_str()))
}

pub(crate) fn path_is_executable_visible_or_host(raw_path: &str) -> bool {
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

pub(crate) fn require_directory(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_directory_error: ValidationError,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(required_error);
    }

    if !path_is_dir_visible_or_host(value) {
        if !path_exists_visible_or_host(value) {
            return Err(missing_error);
        }
        return Err(not_directory_error);
    }

    Ok(())
}

pub(crate) fn require_executable_file(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_executable_error: ValidationError,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(required_error);
    }

    if !path_is_executable_visible_or_host(value) {
        if !path_exists_visible_or_host(value) {
            return Err(missing_error);
        }
        return Err(not_executable_error);
    }

    Ok(())
}

fn path_is_executable_file(path: &str) -> bool {
    let path = Path::new(path);
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
