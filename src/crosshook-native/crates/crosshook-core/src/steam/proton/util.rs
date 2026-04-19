use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn safe_enumerate_directories(
    directory_path: &Path,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf> {
    if !directory_path.is_dir() {
        return Vec::new();
    }

    let entries = match fs::read_dir(directory_path) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(format!(
                "Failed to read directory '{}': {error}",
                directory_path.display()
            ));
            return Vec::new();
        }
    };

    let mut directories = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                }
            }
            Err(error) => {
                diagnostics.push(format!(
                    "Failed to read entry in '{}': {error}",
                    directory_path.display()
                ));
            }
        }
    }

    directories.sort();
    directories
}
