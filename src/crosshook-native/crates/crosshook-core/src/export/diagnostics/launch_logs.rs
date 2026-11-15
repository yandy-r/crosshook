//! Launch log collection for diagnostic bundles.

use std::cmp::Reverse;
use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use nix::dir::Dir;
use nix::fcntl::{open, openat, OFlag};
use nix::sys::stat::{fstat, Mode, SFlag};

use super::logs::{read_tail, MAX_LOG_FILE_BYTES};
use crate::runtime_files;

/// Maximum number of recent launch log files to include per directory.
const MAX_LAUNCH_LOG_FILES: usize = 10;

/// Bundle directory for app launch logs.
const APP_BUNDLE_DIR: &str = "logs/launch";

/// Bundle directory for CLI launch logs.
const CLI_BUNDLE_DIR: &str = "logs/launch-cli";

/// File name prefix the CLI gives its launch logs.
const CLI_LOG_PREFIX: &str = "cli-";

/// Collects the most recent app and CLI launch logs as `(bundle path, data)`.
pub(super) fn collect_launch_logs() -> Vec<(String, Vec<u8>)> {
    collect_launch_logs_with(
        &runtime_files::launch_log_dir_candidates(),
        nix::unistd::geteuid().as_raw(),
    )
}

/// Collects launch logs from every usable directory in `candidates`.
///
/// Directories that resolve to the same device and inode are read once. Files
/// named `cli-*` go to [`CLI_BUNDLE_DIR`], everything else to
/// [`APP_BUNDLE_DIR`]; each bundle folder keeps at most
/// [`MAX_LAUNCH_LOG_FILES`] files, newest first.
fn collect_launch_logs_with(candidates: &[PathBuf], owner: u32) -> Vec<(String, Vec<u8>)> {
    let mut dirs: Vec<OwnedLogDir> = Vec::new();
    for dir in candidates
        .iter()
        .filter_map(|candidate| open_owned_dir(candidate, owner))
    {
        if !dirs.iter().any(|seen| seen.same_dir(&dir)) {
            dirs.push(dir);
        }
    }

    [(false, APP_BUNDLE_DIR), (true, CLI_BUNDLE_DIR)]
        .into_iter()
        .flat_map(|(cli, bundle_dir)| {
            let mut logs: Vec<LogHandle> = dirs
                .iter()
                .flat_map(|dir| dir.recent_handles(|name| name.starts_with(CLI_LOG_PREFIX) == cli))
                .collect();
            keep_newest(&mut logs);
            logs.into_iter().filter_map(move |mut log| {
                let data = read_tail(&mut log.file, MAX_LOG_FILE_BYTES)?;
                Some((format!("{bundle_dir}/{}", log.name), data))
            })
        })
        .collect()
}

/// Collects up to [`MAX_LAUNCH_LOG_FILES`] recent logs from `dir`, newest first.
#[cfg(test)]
fn collect_launch_logs_from(dir: &Path, owner: u32) -> Vec<(String, Vec<u8>)> {
    open_owned_dir(dir, owner).map_or_else(Vec::new, |dir| {
        dir.recent_handles(|_| true)
            .into_iter()
            .filter_map(|mut log| Some((log.name, read_tail(&mut log.file, MAX_LOG_FILE_BYTES)?)))
            .collect()
    })
}

/// A launch log directory opened without following symlinks and verified to
/// be owned by `owner` and not writable by group or others.
struct OwnedLogDir {
    fd: OwnedFd,
    owner: u32,
    dev: u64,
    ino: u64,
}

/// Opens `path` as an [`OwnedLogDir`]; `None` when it is missing, a symlink,
/// not a directory, owned by someone else, or group/other writable.
fn open_owned_dir(path: &Path, owner: u32) -> Option<OwnedLogDir> {
    let fd = open(
        path,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    let stat = fstat(&fd).ok()?;
    if stat.st_uid != owner || stat.st_mode & 0o022 != 0 {
        return None;
    }
    Some(OwnedLogDir {
        fd,
        owner,
        dev: stat.st_dev,
        ino: stat.st_ino,
    })
}

impl OwnedLogDir {
    fn same_dir(&self, other: &Self) -> bool {
        self.dev == other.dev && self.ino == other.ino
    }

    /// Opens the most recently modified `*.log` files whose name satisfies
    /// `wanted`, newest first.
    fn recent_handles(&self, wanted: impl Fn(&str) -> bool) -> Vec<LogHandle> {
        let Some(names) = self.log_names() else {
            return Vec::new();
        };
        // Keeps at most 2 * MAX_LAUNCH_LOG_FILES handles open while scanning.
        let mut logs: Vec<LogHandle> = Vec::with_capacity(2 * MAX_LAUNCH_LOG_FILES);
        for name in names.into_iter().filter(|name| wanted(name)) {
            if let Some(log) = self.open_log(name) {
                logs.push(log);
                if logs.len() == 2 * MAX_LAUNCH_LOG_FILES {
                    keep_newest(&mut logs);
                }
            }
        }
        keep_newest(&mut logs);
        logs
    }

    /// UTF-8 `*.log` entry names in the directory.
    fn log_names(&self) -> Option<Vec<String>> {
        // `Dir` takes ownership of its descriptor, so it gets a duplicate. The
        // duplicate shares the file offset with `self.fd`; `Dir::iter`
        // rewinds it when dropped, so the listing can be repeated.
        let mut dir = Dir::from_fd(self.fd.try_clone().ok()?).ok()?;
        Some(
            dir.iter()
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let name = entry.file_name().to_str().ok()?;
                    let is_log = Path::new(name).extension().is_some_and(|ext| ext == "log");
                    is_log.then(|| name.to_string())
                })
                .collect(),
        )
    }

    /// Opens `name` relative to the verified directory; returns the handle and
    /// its mtime only for a regular file owned by the directory owner.
    fn open_log(&self, name: String) -> Option<LogHandle> {
        let fd = openat(
            &self.fd,
            name.as_str(),
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        let stat = fstat(&fd).ok()?;
        let is_regular =
            SFlag::from_bits_truncate(stat.st_mode & SFlag::S_IFMT.bits()) == SFlag::S_IFREG;
        // `time_t` is 32-bit on some targets; widen without assuming 64-bit.
        #[allow(clippy::useless_conversion)]
        let mtime = (i64::from(stat.st_mtime), i64::from(stat.st_mtime_nsec));
        (is_regular && stat.st_uid == self.owner).then(|| LogHandle {
            name,
            file: File::from(fd),
            mtime,
        })
    }
}

/// An open launch log and the mtime taken from its own handle.
struct LogHandle {
    name: String,
    file: File,
    mtime: (i64, i64),
}

/// Sorts newest first and drops all but [`MAX_LAUNCH_LOG_FILES`] handles.
fn keep_newest(logs: &mut Vec<LogHandle>) {
    logs.sort_by_key(|log| Reverse(log.mtime));
    logs.truncate(MAX_LAUNCH_LOG_FILES);
}

#[cfg(test)]
#[path = "launch_logs_tests.rs"]
mod tests;
