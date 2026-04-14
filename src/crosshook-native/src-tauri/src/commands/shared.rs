use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crosshook_core::platform::host_std_command;

#[tauri::command]
pub fn normalize_host_path(path: String) -> String {
    crosshook_core::platform::normalize_flatpak_host_path(&path)
}

pub fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let file_name = format!("{prefix}-{target_slug}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}

pub fn sanitize_display_path(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) => {
            let path = Path::new(path);
            let home = Path::new(&home);
            match path.strip_prefix(home) {
                Ok(suffix) if suffix.as_os_str().is_empty() => "~/".to_string(),
                Ok(suffix) => format!("~/{}", suffix.display()),
                Err(_) => path.display().to_string(),
            }
        }
        _ => path.to_string(),
    }
}

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

/// Returns true if `entry` contains `prefix` at a path boundary: after a match
/// the next byte must be absent (end of entry), `/`, or `:` (PATH-style lists).
fn environ_entry_contains_prefix_path(entry: &[u8], prefix: &[u8]) -> bool {
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

/// `SIGKILL`s every process whose `/proc/[pid]/environ` references the prefix
/// path.
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
/// expected to fail and are not actionable.
pub fn kill_processes_using_prefix(prefix_path: &Path) {
    // Slugified ad-hoc prefixes are guaranteed ASCII by `slugify`. Assert
    // in debug builds so any future caller passing a non-UTF-8 path is
    // caught immediately rather than silently performing lossy substring
    // matching against `/proc/[pid]/environ` blobs.
    debug_assert!(
        prefix_path.to_str().is_some(),
        "kill_processes_using_prefix expects a UTF-8 path; got non-UTF-8 bytes which will be lossily converted",
    );
    let target_str = prefix_path.to_string_lossy().to_string();
    let target_bytes = target_str.as_bytes();
    if target_bytes.is_empty() {
        return;
    }

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(error) => {
            tracing::warn!(%error, "kill_processes_using_prefix: unable to read /proc");
            return;
        }
    };

    let mut killed = 0u32;
    for entry in proc_dir.flatten() {
        let name_os = entry.file_name();
        let name = match name_os.to_str() {
            Some(s) => s,
            None => continue,
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let environ_path = entry.path().join("environ");
        let environ_bytes = match std::fs::read(&environ_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let target_present = environ_bytes
            .split(|b| *b == b'\0')
            .filter(|entry| !entry.is_empty())
            .any(|entry| environ_entry_contains_prefix_path(entry, target_bytes));
        if !target_present {
            continue;
        }

        tracing::info!(
            pid = %name,
            prefix = %target_str,
            "kill_processes_using_prefix: SIGKILL"
        );
        let _ = host_std_command("kill").arg("-KILL").arg(name).status();
        killed += 1;
    }

    if killed > 0 {
        tracing::info!(
            killed,
            prefix = %target_str,
            "kill_processes_using_prefix: kill sweep complete"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::environ_entry_contains_prefix_path;

    #[test]
    fn prefix_path_rejects_sibling_with_extra_suffix_after_match() {
        let prefix = b"/foo/bar";
        assert!(!environ_entry_contains_prefix_path(
            b"WINEPREFIX=/foo/bar-baz",
            prefix
        ));
    }

    #[test]
    fn prefix_path_accepts_exact_match_and_subdirectory() {
        let prefix = b"/foo/bar";
        assert!(environ_entry_contains_prefix_path(
            b"WINEPREFIX=/foo/bar",
            prefix
        ));
        assert!(environ_entry_contains_prefix_path(b"/foo/bar", prefix));
        assert!(environ_entry_contains_prefix_path(
            b"WINEPREFIX=/foo/bar/pfx",
            prefix
        ));
    }

    #[test]
    fn prefix_path_accepts_colon_separated_path_list() {
        let prefix = b"/foo/bar";
        assert!(environ_entry_contains_prefix_path(
            b"PATH=/a:/foo/bar:/c",
            prefix
        ));
    }
}
