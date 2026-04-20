use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};

pub(super) fn dir_size_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    let entries = match fs::read_dir(path) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    for entry in entries.flatten() {
        let child = entry.path();
        total = total.saturating_add(file_or_dir_size_bytes(&child));
    }

    total
}

pub(super) fn file_or_dir_size_bytes(path: &Path) -> u64 {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    if metadata.file_type().is_symlink() {
        return 0;
    }
    if metadata.is_file() {
        return metadata.len();
    }
    if metadata.is_dir() {
        return dir_size_bytes(path);
    }
    0
}

pub(super) fn normalized_path_string(path: &Path) -> String {
    match fs::canonicalize(path) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

pub(super) fn system_time_to_rfc3339(time: SystemTime) -> String {
    let dt: DateTime<Utc> = time.into();
    dt.to_rfc3339()
}

pub(super) fn has_crosshook_managed_marker(prefix_path: &Path) -> bool {
    prefix_path.join("drive_c/CrossHook").is_dir()
}
