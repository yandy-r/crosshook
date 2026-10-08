use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::metadata::MetadataStatus;

use super::redact_home_paths;

/// Serialized content of the bundle's `metadata-status.json` section.
#[derive(Serialize)]
struct MetadataStatusReport {
    status: MetadataStatus,
    backup_files: Vec<MetadataBackupFileInfo>,
}

#[derive(Serialize)]
struct MetadataBackupFileInfo {
    path: String,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
}

/// Renders the metadata store status and backup file inventory as JSON.
///
/// Missing backup files (e.g. listed by a stale catalog) are kept with
/// `size_bytes`/`modified_at` set to `null` rather than dropped, so the
/// bundle records that the file is gone.
pub(super) fn collect_metadata_status(
    status: &MetadataStatus,
    backup_files: &[PathBuf],
    redact_paths: bool,
) -> String {
    let report = MetadataStatusReport {
        status: status.clone(),
        backup_files: backup_files
            .iter()
            .map(|path| backup_file_info(path, redact_paths))
            .collect(),
    };
    serde_json::to_string_pretty(&report)
        .unwrap_or_else(|error| format!("{{\"error\":\"serialize metadata status: {error}\"}}"))
}

fn backup_file_info(path: &Path, redact_paths: bool) -> MetadataBackupFileInfo {
    let display = path.display().to_string();
    let redacted = if redact_paths {
        redact_home_paths(&display)
    } else {
        display
    };
    match fs::metadata(path) {
        Ok(meta) => MetadataBackupFileInfo {
            path: redacted,
            size_bytes: Some(meta.len()),
            modified_at: meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
        },
        Err(_) => MetadataBackupFileInfo {
            path: redacted,
            size_bytes: None,
            modified_at: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn collect_metadata_status_renders_ok_status_and_backups() {
        let temp = tempdir().unwrap();
        let backup = temp.path().join("metadata-backup-v1.db");
        fs::write(&backup, b"backup").unwrap();

        let text = collect_metadata_status(
            &MetadataStatus::Ok,
            &[backup, temp.path().join("missing.db")],
            false,
        );

        assert!(text.contains("\"state\": \"ok\""));
        assert!(text.contains("metadata-backup-v1.db"));
        assert!(text.contains("\"size_bytes\": 6"));
        assert!(text.contains("\"size_bytes\": null"));
    }

    #[test]
    fn collect_metadata_status_redacts_home_paths() {
        let home = std::env::var("HOME").unwrap();
        let backup = PathBuf::from(&home).join("metadata-backup-v1.db");

        let text = collect_metadata_status(&MetadataStatus::Ok, &[backup], true);

        assert!(!text.contains(&home), "expected home path to be redacted");
        assert!(text.contains("~/metadata-backup-v1.db"));
    }
}
