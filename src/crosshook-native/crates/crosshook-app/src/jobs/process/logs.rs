//! Log-file creation and target slugs.

use std::fs::{DirBuilder, File, OpenOptions, Permissions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nix::libc;

#[cfg(test)]
mod tests;

/// Production log directory shared by every launch kind.
pub(crate) const LOG_DIR: &str = "/tmp/crosshook-logs";

/// Permission bits of every created log file: owner read/write only.
const LOG_FILE_MODE: u32 = 0o600;

/// Permission bits of the log directory: owner only.
const LOG_DIR_MODE: u32 = 0o700;

/// [`create_log_file_in`] with the handle closed, for callers that only need
/// the path (the launch log-dir seam).
///
/// # Errors
/// The I/O error message when the directory or file cannot be created, or
/// the directory is unsafe (see [`create_log_file_in`]).
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
/// `dir` is created with mode `0o700` when missing. It must be a real
/// directory (not a symlink) owned by the effective user; group and other
/// permission bits are then removed, so no other account can replace a log
/// file inside it.
///
/// The file is created with `create_new` (which never follows a symlink at
/// the final component) and a `-N` uniqueness suffix when the
/// same-millisecond name already exists, so two sessions that start within
/// one millisecond never share — and never truncate — one log file. Callers
/// attach child stdio to (clones of) the returned handle instead of
/// reopening the path.
///
/// # Errors
/// The I/O error message when the directory or file cannot be created, or
/// the directory is a symlink, not a directory, or owned by another user.
pub fn create_log_file_in(
    dir: PathBuf,
    prefix: &str,
    target_slug: &str,
) -> Result<(PathBuf, File), String> {
    secure_log_dir(&dir).map_err(|error| format!("log directory {}: {error}", dir.display()))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    create_unique_log(&dir, &format!("{prefix}-{target_slug}-{timestamp}"))
}

/// Creates `<dir>/<stem>.log`, or `<stem>-N.log` when that name is taken,
/// with `create_new`: an existing entry — file or symlink — is never opened.
fn create_unique_log(dir: &Path, stem: &str) -> Result<(PathBuf, File), String> {
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

/// Creates `dir` (mode `0o700`) when missing, then requires a real directory
/// owned by the effective user and strips its group and other bits. The
/// checks and the `chmod` use one `O_NOFOLLOW` handle, so a symlink swapped
/// in at `dir` is refused rather than followed.
fn secure_log_dir(dir: &Path) -> io::Result<()> {
    secure_log_dir_owned_by(dir, nix::unistd::geteuid().as_raw())
}

/// [`secure_log_dir`] with the required owner given (test seam: no test can
/// create a directory owned by another account).
fn secure_log_dir_owned_by(dir: &Path, owner: u32) -> io::Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(LOG_DIR_MODE)
        .create(dir)?;
    let handle = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)
        .map_err(|error| match error.raw_os_error() {
            Some(libc::ELOOP | libc::ENOTDIR) => io::Error::new(
                io::ErrorKind::PermissionDenied,
                "is a symlink or not a directory",
            ),
            _ => error,
        })?;
    let meta = handle.metadata()?;
    if meta.uid() != owner {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "is owned by another user",
        ));
    }
    if meta.mode() & 0o077 != 0 {
        handle.set_permissions(Permissions::from_mode(meta.mode() & 0o7700))?;
    }
    Ok(())
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
