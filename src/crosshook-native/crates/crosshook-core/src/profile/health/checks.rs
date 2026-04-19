use std::fs;
use std::io::ErrorKind;

use super::path_probe::{
    display_path, path_exists_visible_or_host, path_is_dir_visible_or_host,
    path_is_executable_visible_or_host, path_is_file_visible_or_host,
};
use super::types::{HealthIssue, HealthIssueSeverity};

/// Classify a path check result (missing file vs. wrong type / inaccessible) into a `HealthIssue`.
///
/// Returns `None` when the path is healthy (present, correct type, accessible).
pub(super) fn check_file_path(
    field: &str,
    path: &str,
    severity_on_broken: HealthIssueSeverity,
) -> Option<(HealthIssue, bool /* is_stale */)> {
    let display = display_path(path);
    if display.is_empty() {
        return None;
    }

    let original = path.trim();
    if let Ok(meta) = fs::metadata(original) {
        if meta.is_file() {
            return None;
        }
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: display.clone(),
                message: format!("Path exists but is not a file: {display}"),
                remediation: "Select the file itself, not a directory or other path type."
                    .to_string(),
                severity: severity_on_broken,
            },
            false,
        ));
    }

    if path_is_file_visible_or_host(path) {
        return None;
    }

    if path_exists_visible_or_host(path) {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: display.clone(),
                message: format!("Path exists but is not a file: {display}"),
                remediation: "Select the file itself, not a directory or other path type."
                    .to_string(),
                severity: severity_on_broken,
            },
            false,
        ));
    }

    match fs::metadata(original) {
        Ok(meta) if meta.is_file() => {
            // Exists and is a file — healthy
            None
        }
        Ok(_) => {
            // Exists but wrong type (directory, symlink to dir, etc.)
            Some((
                HealthIssue {
                    field: field.to_string(),
                    path: display.to_string(),
                    message: format!("Path exists but is not a file: {display}"),
                    remediation: "Select the file itself, not a directory or other path type."
                        .to_string(),
                    severity: severity_on_broken,
                },
                false,
            ))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            // Missing from disk → Stale
            Some((
                HealthIssue {
                    field: field.to_string(),
                    path: display.to_string(),
                    message: format!("Path does not exist: {display}"),
                    remediation: "Re-browse to the file or verify the path is correct.".to_string(),
                    severity: HealthIssueSeverity::Warning,
                },
                true,
            ))
        }
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Path is not accessible (permission denied): {display}"),
                remediation: "Check file permissions (e.g. chmod a+r).".to_string(),
                severity: severity_on_broken,
            },
            false,
        )),
        Err(err) => {
            // Other I/O errors are treated as broken
            Some((
                HealthIssue {
                    field: field.to_string(),
                    path: display.to_string(),
                    message: format!("Could not access path: {err}"),
                    remediation: "Verify the path is valid and accessible.".to_string(),
                    severity: severity_on_broken,
                },
                false,
            ))
        }
    }
}

/// Check a required file field. Empty path → `Broken`. Missing → `Stale`. Wrong type → `Broken`.
pub(super) fn check_required_file(field: &str, path: &str) -> Option<(HealthIssue, bool)> {
    if path.trim().is_empty() {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("Required field '{field}' is not configured."),
                remediation: format!("Browse to or enter a value for '{field}'."),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }
    check_file_path(field, path, HealthIssueSeverity::Error)
}

/// Check a required directory field. Empty → `Broken`. Missing → `Stale`. Wrong type → `Broken`.
pub(super) fn check_required_directory(field: &str, path: &str) -> Option<(HealthIssue, bool)> {
    let display = display_path(path);
    if display.is_empty() {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("Required field '{field}' is not configured."),
                remediation: format!("Browse to or enter a value for '{field}'."),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    let original = path.trim();
    if let Ok(meta) = fs::metadata(original) {
        if meta.is_dir() {
            return None;
        }
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: display.clone(),
                message: format!("Path exists but is not a directory: {display}"),
                remediation: "Select the directory itself, not a file inside it.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    if path_is_dir_visible_or_host(path) {
        return None;
    }

    if path_exists_visible_or_host(path) {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: display.clone(),
                message: format!("Path exists but is not a directory: {display}"),
                remediation: "Select the directory itself, not a file inside it.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    match fs::metadata(original) {
        Ok(meta) if meta.is_dir() => None,
        Ok(_) => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Path exists but is not a directory: {display}"),
                remediation: "Select the directory itself, not a file inside it.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Directory does not exist: {display}"),
                remediation: "Re-browse to the directory or verify the path is correct."
                    .to_string(),
                severity: HealthIssueSeverity::Warning,
            },
            true,
        )),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Directory is not accessible (permission denied): {display}"),
                remediation: "Check directory permissions (e.g. chmod a+rx).".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
        Err(err) => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Could not access directory: {err}"),
                remediation: "Verify the path is valid and accessible.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
    }
}

/// Check a required executable file field. Empty → `Broken`. Missing → `Stale`. Not executable → `Broken`.
pub(super) fn check_required_executable(field: &str, path: &str) -> Option<(HealthIssue, bool)> {
    let display = display_path(path);
    if display.is_empty() {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("Required field '{field}' is not configured."),
                remediation: format!("Browse to or enter a value for '{field}'."),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    let original = path.trim();
    if let Ok(meta) = fs::metadata(original) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if !meta.is_file() {
                return Some((
                    HealthIssue {
                        field: field.to_string(),
                        path: display.clone(),
                        message: format!("Path exists but is not a file: {display}"),
                        remediation: "Select the executable file itself.".to_string(),
                        severity: HealthIssueSeverity::Error,
                    },
                    false,
                ));
            }
            if meta.permissions().mode() & 0o111 == 0 {
                return Some((
                    HealthIssue {
                        field: field.to_string(),
                        path: display.clone(),
                        message: format!(
                            "File is not executable (no execute permission): {display}"
                        ),
                        remediation: "Run 'chmod +x' on the file to make it executable."
                            .to_string(),
                        severity: HealthIssueSeverity::Error,
                    },
                    false,
                ));
            }
            return None;
        }
        #[cfg(not(unix))]
        {
            if meta.is_file() {
                return None;
            }
            return Some((
                HealthIssue {
                    field: field.to_string(),
                    path: display.clone(),
                    message: format!("Path exists but is not a file: {display}"),
                    remediation: "Select the executable file itself.".to_string(),
                    severity: HealthIssueSeverity::Error,
                },
                false,
            ));
        }
    }

    if path_is_executable_visible_or_host(path) {
        return None;
    }

    if path_is_file_visible_or_host(path) {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: display.clone(),
                message: format!("File is not executable (no execute permission): {display}"),
                remediation: "Run 'chmod +x' on the file to make it executable.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    if path_exists_visible_or_host(path) {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: display.clone(),
                message: format!("Path exists but is not a file: {display}"),
                remediation: "Select the executable file itself.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    match fs::metadata(original) {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if !meta.is_file() {
                    return Some((
                        HealthIssue {
                            field: field.to_string(),
                            path: display.to_string(),
                            message: format!("Path exists but is not a file: {display}"),
                            remediation: "Select the executable file itself.".to_string(),
                            severity: HealthIssueSeverity::Error,
                        },
                        false,
                    ));
                }
                if meta.permissions().mode() & 0o111 == 0 {
                    return Some((
                        HealthIssue {
                            field: field.to_string(),
                            path: display.to_string(),
                            message: format!(
                                "File is not executable (no execute permission): {display}"
                            ),
                            remediation: "Run 'chmod +x' on the file to make it executable."
                                .to_string(),
                            severity: HealthIssueSeverity::Error,
                        },
                        false,
                    ));
                }
                None
            }
            #[cfg(not(unix))]
            {
                if !meta.is_file() {
                    return Some((
                        HealthIssue {
                            field: field.to_string(),
                            path: display.to_string(),
                            message: format!("Path exists but is not a file: {display}"),
                            remediation: "Select the executable file itself.".to_string(),
                            severity: HealthIssueSeverity::Error,
                        },
                        false,
                    ));
                }
                None
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Executable does not exist: {display}"),
                remediation: "Re-browse to the executable or verify the path is correct."
                    .to_string(),
                severity: HealthIssueSeverity::Warning,
            },
            true,
        )),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Executable is not accessible (permission denied): {display}"),
                remediation: "Check file permissions (e.g. chmod a+rx).".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
        Err(err) => Some((
            HealthIssue {
                field: field.to_string(),
                path: display.to_string(),
                message: format!("Could not access executable: {err}"),
                remediation: "Verify the path is valid and accessible.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
    }
}

/// Check an optional path field. Empty → no issue. Missing or inaccessible → `Info`.
pub(super) fn check_optional_path(field: &str, path: &str) -> Option<HealthIssue> {
    let display = display_path(path);
    if display.is_empty() {
        return None;
    }

    if path_is_file_visible_or_host(path) || path_is_dir_visible_or_host(path) {
        return None;
    }

    if path_exists_visible_or_host(path) {
        return None;
    }

    match fs::metadata(path.trim()) {
        Ok(_) => None,
        Err(err) if err.kind() == ErrorKind::NotFound => Some(HealthIssue {
            field: field.to_string(),
            path: display.to_string(),
            message: format!("Optional path does not exist: {display}"),
            remediation: format!("Browse to or clear the '{field}' field if no longer needed."),
            severity: HealthIssueSeverity::Info,
        }),
        Err(_) => Some(HealthIssue {
            field: field.to_string(),
            path: display.to_string(),
            message: format!("Optional path is not accessible: {display}"),
            remediation: format!("Verify the '{field}' path or clear it if no longer needed."),
            severity: HealthIssueSeverity::Info,
        }),
    }
}
