use std::path::{Path, PathBuf};

use super::errors::{map_io_err, InstallError};

/// Return the first non-`.` normal path segment, or `None` if the path has
/// no usable top-level name.
pub(super) fn first_normal_path_component(path: &Path) -> Option<String> {
    use std::path::Component;

    for component in path.components() {
        match component {
            Component::Normal(segment) => {
                let name = segment.to_string_lossy().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
            Component::CurDir => continue,
            _ => return None,
        }
    }

    None
}

fn normalize_archive_relative_path(path: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(normalized)
}

fn validate_link_entry(
    entry_path: &Path,
    entry_type: tar::EntryType,
    link_target: &Path,
) -> Result<(), InstallError> {
    let Some(top_level) = first_normal_path_component(entry_path) else {
        return Err(InstallError::InvalidPath(format!(
            "archive link entry '{}' has no valid top-level directory",
            entry_path.display()
        )));
    };

    let top_level = Path::new(&top_level);
    let invalid_target = || {
        InstallError::InvalidPath(format!(
            "archive link '{}' points outside the install root via '{}'",
            entry_path.display(),
            link_target.display()
        ))
    };

    if link_target.is_absolute() {
        return Err(invalid_target());
    }

    let entry_parent = entry_path.parent().unwrap_or_else(|| Path::new(""));
    let symlink_target = normalize_archive_relative_path(&entry_parent.join(link_target));
    if entry_type.is_symlink() {
        let Some(symlink_target) = symlink_target else {
            return Err(invalid_target());
        };
        if !symlink_target.starts_with(top_level) {
            return Err(invalid_target());
        }
        return Ok(());
    }

    if entry_type.is_hard_link() {
        let direct_target = normalize_archive_relative_path(link_target);
        let is_safe_direct = direct_target
            .as_ref()
            .is_some_and(|candidate| candidate.starts_with(top_level));
        let is_safe_parent_relative = symlink_target
            .as_ref()
            .is_some_and(|candidate| candidate.starts_with(top_level));
        if !(is_safe_direct || is_safe_parent_relative) {
            return Err(invalid_target());
        }
    }

    Ok(())
}

fn validate_entry_path_within_archive_root(
    entry_path: &Path,
    entry_type: tar::EntryType,
    expected_top_level: &mut Option<String>,
) -> Result<(), InstallError> {
    let entry_top_level = first_normal_path_component(entry_path).ok_or_else(|| {
        InstallError::InvalidPath(format!(
            "archive entry '{}' is not under a valid top-level directory",
            entry_path.display()
        ))
    })?;
    let normal_component_count = entry_path
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();

    if normal_component_count == 1 && !entry_type.is_dir() {
        return Err(InstallError::InvalidPath(format!(
            "archive entry '{}' is a root-level file or link; expected a top-level directory wrapper",
            entry_path.display()
        )));
    }

    match expected_top_level {
        Some(expected) if expected == &entry_top_level => Ok(()),
        Some(expected) => Err(InstallError::InvalidPath(format!(
            "archive entry '{}' escapes expected top-level directory '{}' via '{}'",
            entry_path.display(),
            expected,
            entry_top_level
        ))),
        None => {
            *expected_top_level = Some(entry_top_level);
            Ok(())
        }
    }
}

pub(super) fn validate_unpack_result(
    entry_path: &Path,
    unpacked: bool,
) -> Result<(), InstallError> {
    if unpacked {
        Ok(())
    } else {
        Err(InstallError::InvalidPath(format!(
            "archive entry '{}' escapes the install root",
            entry_path.display()
        )))
    }
}

pub(super) fn extract_tar_read_sync<R: std::io::Read>(
    read: R,
    dest_dir: &Path,
) -> Result<String, InstallError> {
    use tar::Archive;

    let mut archive = Archive::new(read);
    let mut top_level_dir = None;
    let entries = archive.entries().map_err(|error| {
        InstallError::Unknown(format!("failed to read archive entries: {error}"))
    })?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|error| {
            InstallError::Unknown(format!("failed to read archive entry: {error}"))
        })?;

        let entry_path = entry
            .path()
            .map_err(|error| {
                InstallError::Unknown(format!("invalid path in archive entry: {error}"))
            })?
            .into_owned();
        validate_entry_path_within_archive_root(
            &entry_path,
            entry.header().entry_type(),
            &mut top_level_dir,
        )?;

        if entry.header().entry_type().is_symlink() || entry.header().entry_type().is_hard_link() {
            let link_target = entry
                .link_name()
                .map_err(|error| {
                    InstallError::Unknown(format!("invalid link target in archive entry: {error}"))
                })?
                .ok_or_else(|| {
                    InstallError::InvalidPath(format!(
                        "archive link entry '{}' is missing a link target",
                        entry_path.display()
                    ))
                })?;
            validate_link_entry(&entry_path, entry.header().entry_type(), &link_target)?;
        }

        let unpacked = entry.unpack_in(dest_dir).map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                InstallError::PermissionDenied(format!(
                    "permission denied extracting to {}: {error}",
                    dest_dir.display()
                ))
            } else {
                InstallError::Unknown(format!("extraction error: {error}"))
            }
        })?;
        validate_unpack_result(&entry_path, unpacked)?;
    }

    top_level_dir.ok_or_else(|| InstallError::Unknown("archive appears to be empty".into()))
}

fn extract_tar_gz_sync(archive_path: &Path, dest_dir: &Path) -> Result<String, InstallError> {
    use flate2::read::GzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|error| {
        map_io_err(
            error,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    extract_tar_read_sync(GzDecoder::new(file), dest_dir)
}

fn extract_tar_xz_sync(archive_path: &Path, dest_dir: &Path) -> Result<String, InstallError> {
    use xz2::read::XzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|error| {
        map_io_err(
            error,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    extract_tar_read_sync(XzDecoder::new(file), dest_dir)
}

pub(super) fn peek_tar_read_top_level_sync<R: std::io::Read>(
    read: R,
) -> Result<String, InstallError> {
    use tar::Archive;

    let mut archive = Archive::new(read);
    let mut top_level_dir = None;
    let entries = archive.entries().map_err(|error| {
        InstallError::Unknown(format!("failed to read archive entries: {error}"))
    })?;

    for entry_result in entries {
        let entry = entry_result.map_err(|error| {
            InstallError::Unknown(format!("failed to read archive entry: {error}"))
        })?;
        let entry_path = entry.path().map_err(|error| {
            InstallError::Unknown(format!("invalid path in archive entry: {error}"))
        })?;
        validate_entry_path_within_archive_root(
            &entry_path,
            entry.header().entry_type(),
            &mut top_level_dir,
        )?;
    }

    top_level_dir.ok_or_else(|| InstallError::Unknown("archive appears to be empty".into()))
}

fn enrich_peek_err(archive_path: &Path, err: InstallError) -> InstallError {
    let size = std::fs::metadata(archive_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    match err {
        InstallError::Unknown(message) => InstallError::Unknown(format!(
            "{message} (archive: {} — on-disk size {size} bytes)",
            archive_path.display()
        )),
        other => other,
    }
}

fn peek_tar_gz_top_level_sync(archive_path: &Path) -> Result<String, InstallError> {
    use flate2::read::GzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|error| {
        map_io_err(
            error,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    peek_tar_read_top_level_sync(GzDecoder::new(file))
        .map_err(|error| enrich_peek_err(archive_path, error))
}

fn peek_tar_xz_top_level_sync(archive_path: &Path) -> Result<String, InstallError> {
    use xz2::read::XzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|error| {
        map_io_err(
            error,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    peek_tar_read_top_level_sync(XzDecoder::new(file))
        .map_err(|error| enrich_peek_err(archive_path, error))
}

fn archive_peek_sync(archive_path: &Path) -> Result<String, InstallError> {
    let name = archive_path
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or("");

    if name.ends_with(".tar.xz") {
        peek_tar_xz_top_level_sync(archive_path)
    } else if name.ends_with(".tar.gz") {
        peek_tar_gz_top_level_sync(archive_path)
    } else {
        Err(InstallError::Unknown(format!(
            "unsupported archive format (expected .tar.gz or .tar.xz): {name}"
        )))
    }
}

fn archive_extract_sync(archive_path: &Path, dest_dir: &Path) -> Result<String, InstallError> {
    let name = archive_path
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or("");

    if name.ends_with(".tar.xz") {
        extract_tar_xz_sync(archive_path, dest_dir)
    } else if name.ends_with(".tar.gz") {
        extract_tar_gz_sync(archive_path, dest_dir)
    } else {
        Err(InstallError::Unknown(format!(
            "unsupported archive format (expected .tar.gz or .tar.xz): {name}"
        )))
    }
}

pub(super) async fn peek_archive(archive_path: PathBuf) -> Result<String, InstallError> {
    tokio::task::spawn_blocking(move || archive_peek_sync(&archive_path))
        .await
        .map_err(|error| InstallError::Unknown(format!("peek task panicked: {error}")))?
}

pub(super) async fn extract_archive(
    archive_path: PathBuf,
    dest_dir: PathBuf,
) -> Result<String, InstallError> {
    tokio::task::spawn_blocking(move || archive_extract_sync(&archive_path, &dest_dir))
        .await
        .map_err(|error| InstallError::Unknown(format!("extraction task panicked: {error}")))?
}

pub(super) fn best_effort_cleanup(temp_path: &Path, partial_dir: Option<&Path>) {
    let _ = std::fs::remove_file(temp_path);
    if let Some(dir) = partial_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
}
