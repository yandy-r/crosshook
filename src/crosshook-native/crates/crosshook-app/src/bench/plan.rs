//! Replay-plan validation: the log must canonicalize to a regular file
//! strictly under the canonical fixture root.

use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use super::env::{ENV_LOG, ENV_RATE, ENV_ROOT};

pub(super) const MAX_REPLAY_BYTES: u64 = 16 * 1024 * 1024;
const MIN_RATE: u32 = 1;
pub(super) const MAX_RATE: u32 = 5000;

/// Replay input. Holds the already-opened log handle so replay never reopens the path
/// (no check/use race); `log` is the canonical path, kept for diagnostics only.
#[derive(Debug)]
pub struct ReplayPlan {
    pub(super) file: File,
    pub(super) log: PathBuf,
    pub(super) rate: u32,
}

impl ReplayPlan {
    /// Canonical path of the replayed log (diagnostics only).
    pub fn log(&self) -> &Path {
        &self.log
    }

    /// Replay rate in lines per second.
    pub fn rate(&self) -> u32 {
        self.rate
    }
}

pub(super) fn parse_rate(raw: Option<&str>) -> Result<u32, String> {
    let Some(raw) = raw else { return Ok(MAX_RATE) };
    match raw.parse::<u32>() {
        Ok(rate) if (MIN_RATE..=MAX_RATE).contains(&rate) => Ok(rate),
        _ => Err(format!(
            "{ENV_RATE} must be an integer in {MIN_RATE}..={MAX_RATE}, got {raw:?}"
        )),
    }
}

fn require_absolute<'a>(name: &str, value: &'a str) -> Result<&'a Path, String> {
    let path = Path::new(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("{name} must be an absolute path"))
    }
}

/// Reject `/` or a root equal to HOME itself. Ancestors of HOME are allowed: the
/// bench harness intentionally runs with an isolated HOME nested under the fixture
/// root, and the runner already verifies real paths, so only the two degenerate
/// breadth cases are guarded here. `root` and `home` must already be canonical.
fn validate_root(root: &Path, home: Option<&Path>) -> Result<(), String> {
    if root == Path::new("/") || home.is_some_and(|h| h == root) {
        return Err(format!("{ENV_ROOT} is too broad (/ or HOME itself)"));
    }
    Ok(())
}

/// Open without following a final-component symlink and check the opened handle
/// (fstat, not path stat): regular file within the size cap.
fn open_log(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("{ENV_LOG} unreadable: {e}"))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("{ENV_LOG} unreadable: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{ENV_LOG} is not a regular file"));
    }
    if meta.len() > MAX_REPLAY_BYTES {
        return Err(format!("{ENV_LOG} exceeds {MAX_REPLAY_BYTES} bytes"));
    }
    Ok(file)
}

/// Validate replay inputs. Log must canonicalize to a regular file strictly under the
/// canonical root (symlink escapes resolve outside the root and are rejected).
/// `home` is the (uncanonicalized) HOME used for the root breadth check.
pub(super) fn resolve_replay_plan(
    root: Option<&str>,
    log: &str,
    rate: Option<&str>,
    home: Option<&Path>,
) -> Result<ReplayPlan, String> {
    let rate = parse_rate(rate)?;
    let root = root.ok_or_else(|| format!("{ENV_ROOT} is required when {ENV_LOG} is set"))?;
    let root = require_absolute(ENV_ROOT, root)?
        .canonicalize()
        .map_err(|e| format!("{ENV_ROOT} unusable: {e}"))?;
    if !root.is_dir() {
        return Err(format!("{ENV_ROOT} is not a directory"));
    }
    validate_root(&root, home.and_then(|h| h.canonicalize().ok()).as_deref())?;
    let requested = require_absolute(ENV_LOG, log)?;
    let log = requested
        .canonicalize()
        .map_err(|e| format!("{ENV_LOG} unusable: {e}"))?;
    if log == root || !log.starts_with(&root) {
        return Err(format!("{ENV_LOG} must be inside {ENV_ROOT}"));
    }
    // Open the configured path, not its canonical target: O_NOFOLLOW then rejects a
    // final-component symlink even when it resolves inside the root.
    let file = open_log(requested)?;
    Ok(ReplayPlan { file, log, rate })
}

#[cfg(test)]
mod tests;
