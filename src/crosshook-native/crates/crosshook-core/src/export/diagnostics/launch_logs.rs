//! Launch log collection for diagnostic bundles.

use std::cmp::Reverse;
use std::ffi::OsString;
use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use nix::dir::Dir;
use nix::fcntl::{open, openat, OFlag};
use nix::sys::stat::{fstat, Mode, SFlag};

use super::logs::{read_tail, MAX_LOG_FILE_BYTES};

/// Maximum number of recent launch log files to include per directory.
const MAX_LAUNCH_LOG_FILES: usize = 10;

/// Launch log directory used by the app.
const APP_LAUNCH_LOG_DIR: &str = "/tmp/crosshook-logs";

/// Launch log directory name the CLI uses under `$XDG_RUNTIME_DIR`.
const CLI_LAUNCH_LOG_DIR_NAME: &str = "crosshook-logs";

/// Bundle directory for app launch logs.
const APP_BUNDLE_DIR: &str = "logs/launch";

/// Bundle directory for CLI launch logs.
const CLI_BUNDLE_DIR: &str = "logs/launch-cli";

/// Collects the most recent app and CLI launch logs as `(bundle path, data)`.
pub(super) fn collect_launch_logs() -> Vec<(String, Vec<u8>)> {
    collect_launch_logs_with(
        Path::new(APP_LAUNCH_LOG_DIR),
        cli_launch_log_dir(std::env::var_os("XDG_RUNTIME_DIR")).as_deref(),
        nix::unistd::geteuid().as_raw(),
    )
}

/// CLI launch log directory for the given `XDG_RUNTIME_DIR` value; `None` when
/// it is unset, empty, or relative.
fn cli_launch_log_dir(runtime_dir: Option<OsString>) -> Option<PathBuf> {
    let runtime_dir = PathBuf::from(runtime_dir?);
    runtime_dir
        .is_absolute()
        .then(|| runtime_dir.join(CLI_LAUNCH_LOG_DIR_NAME))
}

fn collect_launch_logs_with(
    app_dir: &Path,
    cli_dir: Option<&Path>,
    owner: u32,
) -> Vec<(String, Vec<u8>)> {
    let app = open_owned_dir(app_dir, owner);
    let cli = cli_dir
        .and_then(|dir| open_owned_dir(dir, owner))
        .filter(|cli| app.as_ref().is_none_or(|app| !app.same_dir(cli)));

    [(app, APP_BUNDLE_DIR), (cli, CLI_BUNDLE_DIR)]
        .into_iter()
        .filter_map(|(dir, bundle_dir)| Some((dir?, bundle_dir)))
        .flat_map(|(dir, bundle_dir)| {
            dir.recent_logs()
                .into_iter()
                .map(move |(name, data)| (format!("{bundle_dir}/{name}"), data))
        })
        .collect()
}

/// Collects up to [`MAX_LAUNCH_LOG_FILES`] recent logs from `dir`, newest first.
#[cfg(test)]
fn collect_launch_logs_from(dir: &Path, owner: u32) -> Vec<(String, Vec<u8>)> {
    open_owned_dir(dir, owner).map_or_else(Vec::new, |dir| dir.recent_logs())
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

    /// Reads the tails of the most recently modified `*.log` files.
    fn recent_logs(&self) -> Vec<(String, Vec<u8>)> {
        let Some(names) = self.log_names() else {
            return Vec::new();
        };
        // Keeps at most 2 * MAX_LAUNCH_LOG_FILES handles open while scanning.
        let mut logs: Vec<LogHandle> = Vec::with_capacity(2 * MAX_LAUNCH_LOG_FILES);
        for name in names {
            if let Some(log) = self.open_log(name) {
                logs.push(log);
                if logs.len() == 2 * MAX_LAUNCH_LOG_FILES {
                    keep_newest(&mut logs);
                }
            }
        }
        keep_newest(&mut logs);
        logs.into_iter()
            .filter_map(|mut log| Some((log.name, read_tail(&mut log.file, MAX_LOG_FILE_BYTES)?)))
            .collect()
    }

    /// UTF-8 `*.log` entry names in the directory.
    fn log_names(&self) -> Option<Vec<String>> {
        // `Dir` takes ownership of its descriptor, so it gets a duplicate; the
        // duplicate shares the file offset, which only the stream uses.
        let dir = Dir::from_fd(self.fd.try_clone().ok()?).ok()?;
        Some(
            dir.into_iter()
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
