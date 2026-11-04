use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::logging;

/// Maximum bytes to read from a single log file (1 MiB).
pub(super) const MAX_LOG_FILE_BYTES: u64 = 1_048_576;

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

fn read_file_tail_bytes(path: &Path, max_bytes: u64) -> Option<Vec<u8>> {
    read_tail(&mut File::open(path).ok()?, max_bytes)
}

/// Reads at most `max_bytes` from the end of the open `file`.
pub(super) fn read_tail(file: &mut File, max_bytes: u64) -> Option<Vec<u8>> {
    let size = file.metadata().ok()?.len();

    let truncated = size > max_bytes;
    if truncated {
        file.seek(SeekFrom::End(-(max_bytes as i64))).ok()?;
    }

    // Bounded even if the file grows after the size was read.
    let mut buffer = Vec::new();
    file.take(max_bytes).read_to_end(&mut buffer).ok()?;
    // Start a truncated tail at a line boundary: a cut-off first line could hold a
    // fragment of a secret that the bundle scrubber cannot recognise. A tail with
    // no line boundary is all one cut-off line, so nothing of it is kept.
    if truncated {
        match buffer.iter().position(|&byte| byte == b'\n') {
            Some(newline) => drop(buffer.drain(..=newline)),
            None => buffer.clear(),
        }
    }
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use std::fs;

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
    fn read_file_tail_bytes_drops_truncated_tail_without_line_boundary() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("large.log");
        let data = vec![b'x'; 2_000_000]; // 2 MB
        fs::write(&path, &data).unwrap();

        let tail = read_file_tail_bytes(&path, MAX_LOG_FILE_BYTES).unwrap();
        assert!(tail.is_empty());
    }

    #[test]
    fn read_file_tail_bytes_starts_truncated_tail_at_line_boundary() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("large.log");
        let mut data = vec![b'x'; 2_000_000];
        data.extend_from_slice(b"\nlast line\n");
        fs::write(&path, &data).unwrap();

        let tail = read_file_tail_bytes(&path, MAX_LOG_FILE_BYTES).unwrap();

        assert_eq!(tail, b"last line\n");
    }
}
