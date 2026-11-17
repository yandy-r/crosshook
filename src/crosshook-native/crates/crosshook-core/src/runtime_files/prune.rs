//! Retention pruning of old log files.
//!
//! Only regular files owned by the current user whose names end in `.log` or
//! `.gamescope.pid` are ever removed. Entries are inspected and unlinked
//! relative to the verified directory fd without following symlinks, so a
//! planted symlink, FIFO, subdirectory or foreign-owned file is left alone.

use std::cmp::Reverse;
use std::ffi::CString;
use std::os::fd::AsFd;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nix::dir::Dir;
use nix::fcntl::AtFlags;
use nix::sys::stat::{fstatat, SFlag};
use nix::unistd::{geteuid, unlinkat, UnlinkatFlags};

use super::PrivateDir;

/// Default maximum age of a retained file, in days.
const DEFAULT_MAX_AGE_DAYS: u64 = 7;
/// Default maximum number of retained files.
const DEFAULT_MAX_FILES: usize = 200;

/// File name suffixes eligible for pruning.
const PRUNABLE_SUFFIXES: [&str; 2] = [".log", ".gamescope.pid"];

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// How many files, and for how long, the directory keeps them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Files older than this are removed.
    pub max_age: Duration,
    /// Only this many of the newest files are kept. `.log` and
    /// `.gamescope.pid` files share this one budget.
    pub max_files: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(DEFAULT_MAX_AGE_DAYS * SECONDS_PER_DAY),
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

struct Candidate {
    name: CString,
    modified: SystemTime,
}

/// Removes files in `dir` that are older than the policy's age or beyond its
/// count limit, newest kept first. Best effort: any error skips that entry.
/// Returns the number of files removed.
pub fn prune_stale(dir: &PrivateDir, policy: &RetentionPolicy, now: SystemTime) -> usize {
    let mut candidates = collect_candidates(dir);
    candidates.sort_by_key(|candidate| (Reverse(candidate.modified), candidate.name.clone()));

    let removed = candidates
        .iter()
        .enumerate()
        .filter(|(index, candidate)| {
            let age = now
                .duration_since(candidate.modified)
                .unwrap_or(Duration::ZERO);
            *index >= policy.max_files || age > policy.max_age
        })
        .filter(|(_, candidate)| {
            unlinkat(
                dir.as_fd(),
                candidate.name.as_c_str(),
                UnlinkatFlags::NoRemoveDir,
            )
            .is_ok()
        })
        .count();
    tracing::debug!(removed, "pruned stale runtime files");
    removed
}

/// Regular files owned by us with a prunable name, with their mtimes.
fn collect_candidates(dir: &PrivateDir) -> Vec<Candidate> {
    let Ok(owned) = dir.as_fd().try_clone_to_owned() else {
        return Vec::new();
    };
    let Ok(mut entries) = Dir::from_fd(owned) else {
        return Vec::new();
    };
    let euid = geteuid().as_raw();
    // `iter()` rewinds the shared directory offset on drop, so the next listing
    // through the same `PrivateDir` starts from the beginning again.
    entries
        .iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let text = name.to_str().ok()?;
            if !PRUNABLE_SUFFIXES
                .iter()
                .any(|suffix| text.ends_with(suffix))
            {
                return None;
            }
            let stat = fstatat(dir.as_fd(), name, AtFlags::AT_SYMLINK_NOFOLLOW).ok()?;
            let is_regular = stat.st_mode & SFlag::S_IFMT.bits() == SFlag::S_IFREG.bits();
            if !is_regular || stat.st_uid != euid {
                return None;
            }
            let modified = mtime(stat.st_mtime, stat.st_mtime_nsec);
            Some(Candidate {
                name: name.to_owned(),
                modified,
            })
        })
        .collect()
}

fn mtime(seconds: i64, nanos: i64) -> SystemTime {
    let nanos = u32::try_from(nanos).unwrap_or(0);
    if seconds >= 0 {
        UNIX_EPOCH + Duration::new(seconds.unsigned_abs(), nanos)
    } else {
        UNIX_EPOCH - Duration::from_secs(seconds.unsigned_abs())
            + Duration::from_nanos(nanos.into())
    }
}
