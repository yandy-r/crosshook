//! `/proc/*/environ` scan for processes referencing a Wine prefix, and the
//! `SIGKILL` sweep built on it.

use std::path::Path;

use crosshook_core::platform::host_std_command;

/// Returns true if `entry` contains `prefix` at a path boundary: after a match
/// the next byte must be absent (end of entry), `/`, or `:` (PATH-style lists).
pub(super) fn environ_entry_contains_prefix_path(entry: &[u8], prefix: &[u8]) -> bool {
    if prefix.is_empty() || entry.len() < prefix.len() {
        return false;
    }
    (0..=entry.len() - prefix.len()).any(|i| {
        if &entry[i..i + prefix.len()] != prefix {
            return false;
        }
        let after = i + prefix.len();
        after == entry.len() || matches!(entry[after], b'/' | b':')
    })
}

/// Pids of every process whose `/proc/[pid]/environ` references the prefix
/// path, without signalling anything.
///
/// Each `NUL`-separated entry is scanned for the prefix bytes. A match is
/// accepted only at a path boundary: the byte after the prefix must be the
/// end of the entry, `/` (e.g. `<prefix>/pfx`), or `:` (e.g. `PATH`-style
/// lists). This avoids false positives where a shorter prefix would match
/// inside a sibling path such as `<prefix>-other`.
///
/// Proton overrides `WINEPREFIX` for child processes (sets it to
/// `<prefix>/pfx` once it bootstraps the structure), but the parent wrapper,
/// intermediate scripts, and the game executable can each carry the prefix
/// path under different keys (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`,
/// `WINEDLLPATH`, …) with or without the `pfx` suffix.
///
/// Errors reading individual `/proc` entries are silently skipped — exited
/// processes, kernel threads, and processes owned by other users are
/// expected to fail and are not actionable. An empty path or an unreadable
/// `/proc` yields no pids. Under Flatpak only the sandbox `/proc` is visible.
pub fn processes_using_prefix(prefix_path: &Path) -> Vec<u32> {
    // Slugified ad-hoc prefixes are guaranteed ASCII by `slugify`. Assert
    // in debug builds so any future caller passing a non-UTF-8 path is
    // caught immediately rather than silently performing lossy substring
    // matching against `/proc/[pid]/environ` blobs.
    debug_assert!(
        prefix_path.to_str().is_some(),
        "processes_using_prefix expects a UTF-8 path; got non-UTF-8 bytes which will be lossily converted",
    );
    let target_str = prefix_path.to_string_lossy();
    let target_bytes = target_str.as_bytes();
    if target_bytes.is_empty() {
        return Vec::new();
    }

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(error) => {
            tracing::warn!(%error, "processes_using_prefix: unable to read /proc");
            return Vec::new();
        }
    };

    proc_dir
        .flatten()
        .filter_map(|entry| {
            let pid = entry
                .file_name()
                .to_str()
                .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()))?
                .parse::<u32>()
                .ok()?;
            let environ_bytes = std::fs::read(entry.path().join("environ")).ok()?;
            environ_bytes
                .split(|b| *b == b'\0')
                .filter(|entry| !entry.is_empty())
                .any(|entry| environ_entry_contains_prefix_path(entry, target_bytes))
                .then_some(pid)
        })
        .collect()
}

/// `SIGKILL`s every process found by [`processes_using_prefix`] through the
/// host `kill` command.
pub fn kill_processes_using_prefix(prefix_path: &Path) {
    let pids = processes_using_prefix(prefix_path);
    if pids.is_empty() {
        return;
    }
    let target_str = prefix_path.to_string_lossy();
    for pid in &pids {
        tracing::info!(
            pid = %pid,
            prefix = %target_str,
            "kill_processes_using_prefix: SIGKILL"
        );
        let _ = host_std_command("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status();
    }
    tracing::info!(
        killed = pids.len(),
        prefix = %target_str,
        "kill_processes_using_prefix: kill sweep complete"
    );
}
