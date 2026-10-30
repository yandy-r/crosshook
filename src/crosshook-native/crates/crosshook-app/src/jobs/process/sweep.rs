//! `/proc/*/environ` scan for processes whose Wine prefix is a given prefix,
//! and the local `SIGKILL` sweep built on it.
//!
//! Only the values of `WINEPREFIX` and `STEAM_COMPAT_DATA_PATH` are
//! considered, compared as canonical paths (symlinked aliases such as
//! `/home` → `/var/home` match; sibling paths and unrelated variables such as
//! `HOME` never do).

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crosshook_core::platform::is_flatpak;
use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

/// Environment variables whose value names a process's Wine prefix (or its
/// Proton compat-data root, whose `pfx` child is the prefix).
const PREFIX_ENV_KEYS: [&[u8]; 2] = [b"WINEPREFIX", b"STEAM_COMPAT_DATA_PATH"];

/// The canonical forms a prefix can be referenced by.
pub(super) struct PrefixTarget {
    /// The prefix itself.
    path: PathBuf,
    /// Its `pfx` child (when the target is a compat-data root).
    pfx: PathBuf,
}

impl PrefixTarget {
    /// `None` for an empty path.
    pub(super) fn new(prefix_path: &Path) -> Option<Self> {
        if prefix_path.as_os_str().is_empty() {
            return None;
        }
        Some(Self {
            path: canonical_or_raw(prefix_path),
            pfx: canonical_or_raw(&prefix_path.join("pfx")),
        })
    }

    /// Whether the env value `value` names this prefix: the same canonical
    /// path, the compat-data root whose `pfx` is the prefix, or the `pfx` of
    /// the target compat-data root. `cache` memoizes canonicalization per
    /// value within one scan.
    fn matches(&self, value: &Path, cache: &mut HashMap<PathBuf, (PathBuf, PathBuf)>) -> bool {
        if !value.is_absolute() {
            return false;
        }
        let (canonical, canonical_pfx) = cache.entry(value.to_path_buf()).or_insert_with(|| {
            (
                canonical_or_raw(value),
                canonical_or_raw(&value.join("pfx")),
            )
        });
        *canonical == self.path || *canonical_pfx == self.path || *canonical == self.pfx
    }
}

fn canonical_or_raw(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Whether a `NUL`-separated environ block references `target` through one of
/// [`PREFIX_ENV_KEYS`].
pub(super) fn environ_references_prefix(
    environ: &[u8],
    target: &PrefixTarget,
    cache: &mut HashMap<PathBuf, (PathBuf, PathBuf)>,
) -> bool {
    environ.split(|b| *b == b'\0').any(|entry| {
        let Some(eq) = entry.iter().position(|b| *b == b'=') else {
            return false;
        };
        let (key, value) = (&entry[..eq], &entry[eq + 1..]);
        PREFIX_ENV_KEYS.contains(&key) && target.matches(Path::new(OsStr::from_bytes(value)), cache)
    })
}

/// Pids of every visible process whose `WINEPREFIX` or
/// `STEAM_COMPAT_DATA_PATH` names the prefix, without signalling anything.
///
/// Errors reading individual `/proc` entries are skipped — exited processes,
/// kernel threads and processes of other users are expected to fail and are
/// not actionable. An empty path or an unreadable `/proc` yields no pids.
/// Under Flatpak only the sandbox `/proc` is visible, so callers must not
/// treat an empty result as "unused" there.
pub fn processes_using_prefix(prefix_path: &Path) -> Vec<u32> {
    let Some(target) = PrefixTarget::new(prefix_path) else {
        return Vec::new();
    };
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(error) => {
            tracing::warn!(%error, "processes_using_prefix: unable to read /proc");
            return Vec::new();
        }
    };
    let mut cache = HashMap::new();
    proc_dir
        .flatten()
        .filter_map(|entry| {
            let pid = entry
                .file_name()
                .to_str()
                .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()))?
                .parse::<u32>()
                .ok()?;
            let environ = std::fs::read(entry.path().join("environ")).ok()?;
            environ_references_prefix(&environ, &target, &mut cache).then_some(pid)
        })
        .collect()
}

/// CrossHook's own pid and every ancestor up to init: never signalled.
pub(super) fn protected_pids() -> HashSet<u32> {
    let mut protected = HashSet::new();
    let mut pid = std::process::id();
    while pid > 0 && protected.insert(pid) {
        let Some(parent) = parent_pid(pid) else {
            break;
        };
        pid = parent;
    }
    protected
}

/// Field 4 of `/proc/<pid>/stat` (after the parenthesized command name).
fn parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// `SIGKILL`s every process found by [`processes_using_prefix`], locally,
/// except CrossHook itself and its ancestors. Skipped under Flatpak, where
/// the scan sees only the sandbox and host pids cannot be addressed.
pub fn kill_processes_using_prefix(prefix_path: &Path) {
    kill_processes_using_prefix_with(prefix_path, is_flatpak(), &protected_pids());
}

/// [`kill_processes_using_prefix`] with an explicit Flatpak decision and set
/// of pids that are never signalled.
pub(super) fn kill_processes_using_prefix_with(
    prefix_path: &Path,
    flatpak: bool,
    protected: &HashSet<u32>,
) {
    if flatpak {
        tracing::debug!("kill_processes_using_prefix: skipped under Flatpak");
        return;
    }
    let pids: Vec<u32> = processes_using_prefix(prefix_path)
        .into_iter()
        .filter(|pid| !protected.contains(pid))
        .collect();
    if pids.is_empty() {
        return;
    }
    let target_str = prefix_path.to_string_lossy();
    for pid in &pids {
        let Ok(raw) = i32::try_from(*pid) else {
            continue;
        };
        tracing::info!(pid = %pid, prefix = %target_str, "kill_processes_using_prefix: SIGKILL");
        match kill(Pid::from_raw(raw), Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => {}
            Err(error) => {
                tracing::warn!(pid = %pid, %error, "kill_processes_using_prefix: kill failed");
            }
        }
    }
    tracing::info!(
        killed = pids.len(),
        prefix = %target_str,
        "kill_processes_using_prefix: kill sweep complete"
    );
}
