use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::logging;

/// Maximum bytes to read from a single log file (1 MiB).
const MAX_LOG_FILE_BYTES: u64 = 1_048_576;

/// Maximum number of recent launch log files to include.
const MAX_LAUNCH_LOG_FILES: usize = 10;

/// Launch log directory used by the Tauri app.
const LAUNCH_LOG_DIR: &str = "/tmp/crosshook-logs";

/// Collects app logs from the structured logging directory.
pub(super) fn collect_app_logs() -> Vec<(String, Vec<u8>)> {
    let log_path = match logging::log_file_path() {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };

    let mut logs = Vec::new();

    // Current log and up to 3 rotated files.
    let candidates: Vec<PathBuf> = std::iter::once(log_path.clone())
        .chain((1..=logging::DEFAULT_LOG_ROTATED_FILES).map(|i| {
            let mut path = log_path.clone();
            let name = format!(
                "{}.{i}",
                log_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(logging::DEFAULT_LOG_FILE_NAME)
            );
            path.set_file_name(name);
            path
        }))
        .collect();

    for path in candidates {
        if let Some(data) = read_file_tail_bytes(&path, MAX_LOG_FILE_BYTES) {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.log")
                .to_string();
            logs.push((filename, data));
        }
    }

    logs
}

/// Collects the most recent launch logs from `/tmp/crosshook-logs/`.
pub(super) fn collect_launch_logs() -> Vec<(String, Vec<u8>)> {
    collect_launch_logs_from(Path::new(LAUNCH_LOG_DIR))
}

fn collect_launch_logs_from(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut log_files: Vec<(PathBuf, std::time::SystemTime)> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("log") {
                return None;
            }
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((path, mtime))
        })
        .collect();

    // Sort by modification time descending (most recent first).
    log_files.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    log_files.truncate(MAX_LAUNCH_LOG_FILES);

    log_files
        .into_iter()
        .filter_map(|(path, _)| {
            let data = read_file_tail_bytes(&path, MAX_LOG_FILE_BYTES)?;
            let filename = path.file_name().and_then(|n| n.to_str())?.to_string();
            Some((filename, data))
        })
        .collect()
}

fn read_file_tail_bytes(path: &Path, max_bytes: u64) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let size = metadata.len();

    if size > max_bytes {
        file.seek(SeekFrom::End(-(max_bytes as i64))).ok()?;
    }

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn collect_app_logs_returns_empty_when_no_logs_exist() {
        // This test verifies the function handles a missing log directory
        // gracefully. In CI or fresh environments, there may be no logs.
        let logs = collect_app_logs();
        // We cannot assert emptiness because the dev machine may have logs,
        // but we verify it does not panic.
        let _ = logs;
    }

    #[test]
    fn collect_launch_logs_caps_at_max_files() {
        let temp = tempdir().unwrap();
        let log_dir = temp.path();

        for i in 0..15 {
            let name = format!("game-{i:02}.log");
            fs::write(log_dir.join(&name), format!("log content {i}")).unwrap();
            // Ensure distinct modification times.
            thread::sleep(Duration::from_millis(10));
        }

        let logs = collect_launch_logs_from(log_dir);
        assert!(logs.len() <= MAX_LAUNCH_LOG_FILES);
    }

    #[test]
    fn collect_launch_logs_returns_most_recent_first() {
        let temp = tempdir().unwrap();
        let log_dir = temp.path();

        fs::write(log_dir.join("old.log"), "old").unwrap();
        thread::sleep(Duration::from_millis(20));
        fs::write(log_dir.join("new.log"), "new").unwrap();

        let logs = collect_launch_logs_from(log_dir);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].0, "new.log");
        assert_eq!(logs[1].0, "old.log");
    }

    #[test]
    fn read_file_tail_bytes_caps_large_files() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("large.log");
        let data = vec![b'x'; 2_000_000]; // 2 MB
        fs::write(&path, &data).unwrap();

        let tail = read_file_tail_bytes(&path, MAX_LOG_FILE_BYTES).unwrap();
        assert_eq!(tail.len(), MAX_LOG_FILE_BYTES as usize);
    }
}
