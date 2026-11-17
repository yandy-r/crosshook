//! Private log file opens relative to the verified directory.
//!
//! New log files are created with `openat` relative to the verified
//! [`PrivateDir`] fd, so the directory is never re-resolved by path. Existing
//! files are opened with `O_NOFOLLOW` and must be regular files owned by the
//! current user.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use nix::errno::Errno;
use nix::fcntl::{openat, OFlag};
use nix::libc;
use nix::sys::stat::Mode;
use nix::unistd::geteuid;

use super::{PrivateDir, RuntimeDirError};

/// Permission bits of a new log file (owner read/write only).
const LOG_FILE_MODE: u32 = 0o600;
/// Highest numeric suffix tried after the bare `<stem>.log` name.
const MAX_NAME_SUFFIX: u32 = 999;

/// Creates a new, exclusive `0600` log file `<stem>.log` in `dir`.
///
/// When that name is taken, `<stem>-1.log` up to `<stem>-999.log` are tried.
/// A name that exists (including a planted symlink) is skipped, never opened or
/// truncated. A `stem` that is empty, starts with `.`, or contains `/` or NUL
/// is refused as [`RuntimeDirError::UnsafeFile`], so the file always lands
/// directly inside `dir`.
pub fn create_unique_log_file(
    dir: &PrivateDir,
    stem: &str,
) -> Result<(PathBuf, File), RuntimeDirError> {
    if !is_safe_stem(stem) {
        return Err(RuntimeDirError::UnsafeFile);
    }
    for attempt in 0..=MAX_NAME_SUFFIX {
        let name = if attempt == 0 {
            format!("{stem}.log")
        } else {
            format!("{stem}-{attempt}.log")
        };
        match open_new_file(dir, &name) {
            Ok(fd) => return Ok((dir.path().join(&name), File::from(fd))),
            Err(Errno::EEXIST | Errno::ELOOP) => continue,
            Err(errno) => return Err(io::Error::from_raw_os_error(errno as i32).into()),
        }
    }
    Err(RuntimeDirError::NamesExhausted)
}

/// Opens an existing log file for appending without following a symlink.
///
/// The file is never created. It must be a regular file owned by the current
/// user. `O_NONBLOCK` makes a FIFO at the path fail fast instead of blocking;
/// it has no effect on regular files.
pub fn open_private_append(path: &Path) -> Result<File, RuntimeDirError> {
    let file = OpenOptions::new()
        .append(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(classify_open_error)?;
    check_regular_owned(&file)?;
    Ok(file)
}

/// Opens an existing log file for reading without following a symlink and
/// without blocking on a FIFO.
///
/// It must be a regular file owned by the current user.
pub fn open_private_read(path: &Path) -> Result<File, RuntimeDirError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(classify_open_error)?;
    check_regular_owned(&file)?;
    Ok(file)
}

fn is_safe_stem(stem: &str) -> bool {
    !stem.is_empty() && !stem.starts_with('.') && !stem.contains(['/', '\0'])
}

fn open_new_file(dir: &PrivateDir, name: &str) -> Result<OwnedFd, Errno> {
    openat(
        dir,
        name,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(LOG_FILE_MODE),
    )
}

/// A symlink at the final component (`ELOOP` under `O_NOFOLLOW`) is an unsafe file.
fn classify_open_error(error: io::Error) -> RuntimeDirError {
    if error.raw_os_error() == Some(Errno::ELOOP as i32) {
        RuntimeDirError::UnsafeFile
    } else {
        RuntimeDirError::Io(error)
    }
}

/// Requires the open handle to be a regular file owned by the current user.
fn check_regular_owned(file: &File) -> Result<(), RuntimeDirError> {
    let metadata = file.metadata()?;
    if metadata.is_file() && metadata.uid() == geteuid().as_raw() {
        Ok(())
    } else {
        Err(RuntimeDirError::UnsafeFile)
    }
}
