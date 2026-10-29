//! Log-file creation and target slugs.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Production log directory shared by every launch kind.
pub(crate) const LOG_DIR: &str = "/tmp/crosshook-logs";

/// Permission bits of every created log file: owner read/write only.
const LOG_FILE_MODE: u32 = 0o600;

/// Creates an empty `/tmp/crosshook-logs/<prefix>-<slug>-<millis>.log` file
/// and returns its path.
///
/// # Errors
/// The I/O error message when the directory or file cannot be created.
pub fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    create_log_path_in(PathBuf::from(LOG_DIR), prefix, target_slug)
}

/// [`create_log_path`] in an explicit directory, so services and tests can
/// point log creation anywhere (the log-dir seam). Same file semantics as
/// [`create_log_file_in`]; the handle is closed.
///
/// # Errors
/// The I/O error message when the directory or file cannot be created.
pub fn create_log_path_in(
    dir: PathBuf,
    prefix: &str,
    target_slug: &str,
) -> Result<PathBuf, String> {
    create_log_file_in(dir, prefix, target_slug).map(|(path, _file)| path)
}

/// Creates `<dir>/<prefix>-<slug>-<millis>.log` with mode `0o600` and returns
/// its path together with the open, write-only handle.
///
/// The file is created with `create_new` (which never follows a symlink at
/// the final component) and a `-N` uniqueness suffix when the
/// same-millisecond name already exists, so two sessions that start within
/// one millisecond never share — and never truncate — one log file. Callers
/// attach child stdio to (clones of) the returned handle instead of
/// reopening the path.
///
/// # Errors
/// The I/O error message when the directory or file cannot be created.
pub fn create_log_file_in(
    dir: PathBuf,
    prefix: &str,
    target_slug: &str,
) -> Result<(PathBuf, File), String> {
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let stem = format!("{prefix}-{target_slug}-{timestamp}");
    for suffix in 0..1000 {
        let file_name = if suffix == 0 {
            format!("{stem}.log")
        } else {
            format!("{stem}-{suffix}.log")
        };
        let log_path = dir.join(file_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(LOG_FILE_MODE)
            .open(&log_path)
        {
            Ok(file) => return Ok((log_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(format!(
        "no free log file name for `{stem}` in {}",
        dir.display()
    ))
}

/// Lowercases ASCII alphanumerics, maps everything else to `-`, trims `-`;
/// returns `fallback` when nothing remains.
pub fn slugify_target(name: &str, fallback: &str) -> String {
    let slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
