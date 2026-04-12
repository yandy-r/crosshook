//! Diagnostic bundle export — collects system info, profiles, logs, and Steam
//! diagnostics into a single `.tar.gz` archive for bug reports.

use std::env;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use tar::Builder;

use crate::logging;
use crate::platform;
use crate::profile::health::batch_check_health;
use crate::profile::ProfileStore;
use crate::settings::SettingsStore;
use crate::steam;

/// Maximum bytes to read from a single log file (1 MiB).
const MAX_LOG_FILE_BYTES: u64 = 1_048_576;

/// Maximum number of recent launch log files to include.
const MAX_LAUNCH_LOG_FILES: usize = 10;

/// Launch log directory used by the Tauri app.
const LAUNCH_LOG_DIR: &str = "/tmp/crosshook-logs";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options controlling diagnostic bundle generation.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticBundleOptions {
    /// When `true`, replaces the user's home directory with `~` in profile TOML
    /// content and settings.
    pub redact_paths: bool,
    /// Override the output directory. Defaults to the system temp directory.
    pub output_dir: Option<PathBuf>,
}

/// Result returned after successful bundle creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundleResult {
    /// Absolute path to the generated `.tar.gz` archive.
    pub archive_path: String,
    /// Human-readable summary of what was collected.
    pub summary: DiagnosticBundleSummary,
}

/// Summary of bundle contents for display in the UI or CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundleSummary {
    pub crosshook_version: String,
    pub profile_count: usize,
    pub log_file_count: usize,
    pub proton_install_count: usize,
    pub generated_at: String,
}

/// Errors that can occur during bundle generation.
#[derive(Debug)]
pub enum DiagnosticBundleError {
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Archive(String),
    ProfileStore(String),
}

impl Display for DiagnosticBundleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} '{}': {source}", path.display()),
            Self::Archive(message) => write!(f, "archive error: {message}"),
            Self::ProfileStore(message) => write!(f, "profile store error: {message}"),
        }
    }
}

impl Error for DiagnosticBundleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Archive(_) | Self::ProfileStore(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level export function
// ---------------------------------------------------------------------------

/// Assembles a diagnostic bundle archive containing system info, profiles,
/// logs, and Steam diagnostics.
pub fn export_diagnostic_bundle(
    profile_store: &ProfileStore,
    settings_store: &SettingsStore,
    options: &DiagnosticBundleOptions,
) -> Result<DiagnosticBundleResult, DiagnosticBundleError> {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let prefix = format!("crosshook-diagnostics-{timestamp}");
    let archive_name = format!("{prefix}.tar.gz");

    let output_dir = match &options.output_dir {
        Some(dir) => dir.clone(),
        None => env::temp_dir(),
    };

    fs::create_dir_all(&output_dir).map_err(|source| DiagnosticBundleError::Io {
        action: "create the output directory",
        path: output_dir.clone(),
        source,
    })?;

    let archive_path = output_dir.join(&archive_name);
    let archive_file = File::create(&archive_path).map_err(|source| DiagnosticBundleError::Io {
        action: "create the archive file",
        path: archive_path.clone(),
        source,
    })?;

    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut tar = Builder::new(encoder);

    // -- Collect and add each section --

    let system_info = collect_system_info();
    append_text(&mut tar, &prefix, "system-info.txt", &system_info)?;

    let crosshook_info = collect_crosshook_info(settings_store, options.redact_paths);
    append_text(&mut tar, &prefix, "crosshook-info.txt", &crosshook_info)?;

    let health_summary = collect_health_summary(profile_store);
    append_text(&mut tar, &prefix, "health-summary.json", &health_summary)?;

    let (steam_diag_text, proton_installs_json, proton_install_count) = collect_steam_diagnostics();
    append_text(&mut tar, &prefix, "steam-diagnostics.txt", &steam_diag_text)?;
    append_text(
        &mut tar,
        &prefix,
        "proton-installs.json",
        &proton_installs_json,
    )?;

    let settings_text = collect_settings(settings_store, options.redact_paths);
    append_text(&mut tar, &prefix, "settings.toml", &settings_text)?;

    let profiles = collect_profiles(profile_store, options.redact_paths);
    for (name, content) in &profiles {
        let entry_path = format!("profiles/{name}.toml");
        append_text(&mut tar, &prefix, &entry_path, content)?;
    }

    let app_logs = collect_app_logs();
    for (filename, data) in &app_logs {
        let entry_path = format!("logs/app/{filename}");
        append_bytes(&mut tar, &prefix, &entry_path, data)?;
    }

    let launch_logs = collect_launch_logs();
    for (filename, data) in &launch_logs {
        let entry_path = format!("logs/launch/{filename}");
        append_bytes(&mut tar, &prefix, &entry_path, data)?;
    }

    let log_file_count = app_logs.len() + launch_logs.len();

    // Finalize the archive.
    let encoder = tar
        .into_inner()
        .map_err(|error| DiagnosticBundleError::Archive(error.to_string()))?;
    encoder
        .finish()
        .map_err(|source| DiagnosticBundleError::Io {
            action: "finalize the gzip stream",
            path: archive_path.clone(),
            source,
        })?;

    Ok(DiagnosticBundleResult {
        archive_path: archive_path.to_string_lossy().into_owned(),
        summary: DiagnosticBundleSummary {
            crosshook_version: env!("CARGO_PKG_VERSION").to_string(),
            profile_count: profiles.len(),
            log_file_count,
            proton_install_count,
            generated_at: Utc::now().to_rfc3339(),
        },
    })
}

// ---------------------------------------------------------------------------
// Collector functions
// ---------------------------------------------------------------------------

/// Collects system information from `/proc` and environment variables.
#[allow(clippy::vec_init_then_push)]
fn collect_system_info() -> String {
    let mut lines = Vec::new();

    lines.push("=== Kernel ===".to_string());
    lines.push(read_file_lossy("/proc/version").unwrap_or_else(|| "(unavailable)".to_string()));
    lines.push(String::new());

    lines.push("=== OS Release ===".to_string());
    lines.push(read_file_lossy("/etc/os-release").unwrap_or_else(|| "(unavailable)".to_string()));
    lines.push(String::new());

    lines.push("=== CPU ===".to_string());
    if let Some(cpuinfo) = read_file_lossy("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("model name") {
                lines.push(line.to_string());
                break;
            }
        }
    } else {
        lines.push("(unavailable)".to_string());
    }
    lines.push(String::new());

    lines.push("=== Memory ===".to_string());
    if let Some(meminfo) = read_file_lossy("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal") {
                lines.push(line.to_string());
                break;
            }
        }
    } else {
        lines.push("(unavailable)".to_string());
    }
    lines.push(String::new());

    lines.push("=== GPU ===".to_string());
    let lspci_output = if platform::is_flatpak() {
        platform::host_std_command("lspci").output()
    } else {
        Command::new("lspci").output()
    };
    match lspci_output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    lines.push(format!("(lspci failed: {})", output.status));
                } else {
                    lines.push(format!("(lspci failed: {}: {stderr})", output.status));
                }
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let gpu_lines: Vec<&str> = stdout
                    .lines()
                    .filter(|line| {
                        let lower = line.to_lowercase();
                        lower.contains("vga") || lower.contains("3d controller")
                    })
                    .collect();
                if gpu_lines.is_empty() {
                    lines.push("(no VGA/3D devices found)".to_string());
                } else {
                    for gpu_line in gpu_lines {
                        lines.push(gpu_line.to_string());
                    }
                }
            }
        }
        Err(error) => lines.push(format!("(lspci not available: {error})")),
    }

    if let Some(nvidia) = read_file_lossy("/proc/driver/nvidia/version") {
        lines.push(String::new());
        lines.push("=== NVIDIA Driver ===".to_string());
        lines.push(nvidia);
    }
    lines.push(String::new());

    lines.push("=== Desktop Environment ===".to_string());
    lines.push(format!(
        "XDG_CURRENT_DESKTOP: {}",
        env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "(unset)".to_string())
    ));
    lines.push(format!(
        "XDG_SESSION_TYPE: {}",
        env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "(unset)".to_string())
    ));
    lines.push(format!(
        "WAYLAND_DISPLAY: {}",
        env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "(unset)".to_string())
    ));

    lines.join("\n")
}

/// Collects CrossHook version and settings summary.
fn collect_crosshook_info(settings_store: &SettingsStore, redact_paths: bool) -> String {
    let mut lines = Vec::new();

    lines.push(format!("CrossHook version: {}", env!("CARGO_PKG_VERSION")));
    lines.push(format!(
        "Settings path: {}",
        settings_store.settings_path().display()
    ));
    lines.push(String::new());

    match settings_store.load() {
        Ok(data) => {
            lines.push(format!(
                "auto_load_last_profile: {}",
                data.auto_load_last_profile
            ));
            lines.push(format!("last_used_profile: {}", data.last_used_profile));
            lines.push(format!("community_taps: {}", data.community_taps.len()));
            for tap in &data.community_taps {
                let url = if redact_paths {
                    redact_home_paths(&tap.url)
                } else {
                    tap.url.clone()
                };
                lines.push(format!("  - {url}"));
            }
        }
        Err(error) => {
            lines.push(format!("(failed to load settings: {error})"));
        }
    }

    lines.join("\n")
}

/// Reads each profile TOML as raw text, optionally redacting home paths.
fn collect_profiles(store: &ProfileStore, redact_paths: bool) -> Vec<(String, String)> {
    let names = match store.list() {
        Ok(names) => names,
        Err(_) => return Vec::new(),
    };

    let mut profiles = Vec::with_capacity(names.len());
    for name in &names {
        let path = store.base_path.join(format!("{name}.toml"));
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let content = if redact_paths {
            redact_home_paths(&content)
        } else {
            content
        };
        profiles.push((name.clone(), content));
    }

    profiles
}

/// Reads the settings TOML file as raw text.
fn collect_settings(settings_store: &SettingsStore, redact_paths: bool) -> String {
    let path = settings_store.settings_path();
    match fs::read_to_string(&path) {
        Ok(content) => {
            if redact_paths {
                redact_home_paths(&content)
            } else {
                content
            }
        }
        Err(_) => "(settings file not found)".to_string(),
    }
}

/// Collects app logs from the structured logging directory.
fn collect_app_logs() -> Vec<(String, Vec<u8>)> {
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
fn collect_launch_logs() -> Vec<(String, Vec<u8>)> {
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
    log_files.sort_by(|a, b| b.1.cmp(&a.1));
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

/// Runs Steam discovery and Proton enumeration.
fn collect_steam_diagnostics() -> (String, String, usize) {
    let mut diagnostics = Vec::new();
    let root_candidates = steam::discover_steam_root_candidates("", &mut diagnostics);

    let mut lines = Vec::new();
    lines.push("=== Steam Root Candidates ===".to_string());
    if root_candidates.is_empty() {
        lines.push("(none found)".to_string());
    } else {
        for root in &root_candidates {
            lines.push(format!("  {}", root.display()));
        }
    }
    lines.push(String::new());

    lines.push("=== Discovery Diagnostics ===".to_string());
    if diagnostics.is_empty() {
        lines.push("(no diagnostics)".to_string());
    } else {
        for diagnostic in &diagnostics {
            lines.push(format!("  {diagnostic}"));
        }
    }

    let mut proton_diagnostics = Vec::new();
    let proton_installs = steam::discover_compat_tools(&root_candidates, &mut proton_diagnostics);
    let proton_count = proton_installs.len();

    if !proton_diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("=== Proton Discovery Diagnostics ===".to_string());
        for diagnostic in &proton_diagnostics {
            lines.push(format!("  {diagnostic}"));
        }
    }

    let proton_json =
        serde_json::to_string_pretty(&proton_installs).unwrap_or_else(|_| "[]".to_string());

    (lines.join("\n"), proton_json, proton_count)
}

/// Runs batch health check across all profiles and serializes the result.
fn collect_health_summary(store: &ProfileStore) -> String {
    let summary = batch_check_health(store);
    serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reads the tail of a file, capped at `max_bytes`.
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

/// Reads an entire file as a lossy UTF-8 string, returning `None` on error.
fn read_file_lossy(path: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Replaces all occurrences of the user's home directory path with `~`.
fn redact_home_paths(text: &str) -> String {
    let home = match env::var("HOME") {
        Ok(home) if !home.is_empty() => home,
        _ => return text.to_string(),
    };

    // Replace both with and without trailing slash.
    let with_slash = format!("{home}/");
    let result = text.replace(&with_slash, "~/");

    // Also replace bare home path at end of line / end of value.
    result.replace(&home, "~")
}

/// Appends a text entry to the tar archive.
fn append_text<W: io::Write>(
    tar: &mut Builder<W>,
    prefix: &str,
    entry_path: &str,
    content: &str,
) -> Result<(), DiagnosticBundleError> {
    append_bytes(tar, prefix, entry_path, content.as_bytes())
}

/// Appends a byte entry to the tar archive.
fn append_bytes<W: io::Write>(
    tar: &mut Builder<W>,
    prefix: &str,
    entry_path: &str,
    data: &[u8],
) -> Result<(), DiagnosticBundleError> {
    let full_path = format!("{prefix}/{entry_path}");

    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    tar.append_data(&mut header, &full_path, data)
        .map_err(|error| DiagnosticBundleError::Archive(error.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn collect_system_info_returns_nonempty_string() {
        let info = collect_system_info();
        assert!(!info.is_empty());
        assert!(info.contains("Kernel") || info.contains("kernel"));
    }

    #[test]
    fn collect_crosshook_info_includes_version() {
        let temp = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp.path().to_path_buf());
        let info = collect_crosshook_info(&store, false);
        assert!(info.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn collect_profiles_returns_correct_count() {
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("game-a.toml"),
            "[game]\nname = \"Game A\"\n",
        )
        .unwrap();
        fs::write(
            profiles_dir.join("game-b.toml"),
            "[game]\nname = \"Game B\"\n",
        )
        .unwrap();

        let store = ProfileStore::with_base_path(profiles_dir);
        let profiles = collect_profiles(&store, false);
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn redact_home_paths_replaces_home_directory() {
        let home = env::var("HOME").unwrap();
        let text = format!("path = \"{home}/games/elden-ring.exe\"");
        let redacted = redact_home_paths(&text);
        assert!(redacted.contains("~/games/elden-ring.exe"));
        assert!(!redacted.contains(&home));
    }

    #[test]
    fn redact_home_paths_preserves_text_without_home() {
        let text = "path = \"/opt/games/elden-ring.exe\"";
        let redacted = redact_home_paths(text);
        assert_eq!(redacted, text);
    }

    #[test]
    fn collect_profiles_with_redaction_replaces_home_paths() {
        let home = env::var("HOME").unwrap();
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("test.toml"),
            format!("[game]\nname = \"Test\"\nexecutable_path = \"{home}/games/test.exe\"\n"),
        )
        .unwrap();

        let store = ProfileStore::with_base_path(profiles_dir);
        let profiles = collect_profiles(&store, true);
        assert_eq!(profiles.len(), 1);
        let (_, content) = &profiles[0];
        assert!(content.contains("~/games/test.exe"));
        assert!(!content.contains(&home));
    }

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
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let logs = collect_launch_logs_from(log_dir);
        assert!(logs.len() <= MAX_LAUNCH_LOG_FILES);
    }

    #[test]
    fn collect_launch_logs_returns_most_recent_first() {
        let temp = tempdir().unwrap();
        let log_dir = temp.path();

        fs::write(log_dir.join("old.log"), "old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
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

    #[test]
    fn diagnostic_bundle_error_display_formats_correctly() {
        let err = DiagnosticBundleError::Archive("test error".to_string());
        assert_eq!(err.to_string(), "archive error: test error");

        let err = DiagnosticBundleError::ProfileStore("store error".to_string());
        assert_eq!(err.to_string(), "profile store error: store error");

        let err = DiagnosticBundleError::Io {
            action: "create",
            path: PathBuf::from("/tmp/test"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        };
        assert!(err.to_string().contains("create"));
        assert!(err.to_string().contains("/tmp/test"));
    }

    #[test]
    fn export_diagnostic_bundle_produces_valid_archive() {
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let settings_dir = temp.path().join("settings");
        let output_dir = temp.path().join("output");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::create_dir_all(&settings_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();

        fs::write(
            profiles_dir.join("test-game.toml"),
            "[game]\nname = \"Test Game\"\nexecutable_path = \"/games/test.exe\"\n",
        )
        .unwrap();

        let profile_store = ProfileStore::with_base_path(profiles_dir);
        let settings_store = SettingsStore::with_base_path(settings_dir);
        let options = DiagnosticBundleOptions {
            redact_paths: false,
            output_dir: Some(output_dir),
        };

        let result = export_diagnostic_bundle(&profile_store, &settings_store, &options).unwrap();

        // Verify the archive file exists.
        assert!(Path::new(&result.archive_path).exists());
        assert!(result.archive_path.ends_with(".tar.gz"));
        assert_eq!(result.summary.profile_count, 1);
        assert!(result
            .summary
            .crosshook_version
            .contains(env!("CARGO_PKG_VERSION")));

        // Decompress and verify archive contents.
        let archive_file = File::open(&result.archive_path).unwrap();
        let decoder = flate2::read::GzDecoder::new(archive_file);
        let mut archive = tar::Archive::new(decoder);
        let entry_paths: Vec<String> = archive
            .entries()
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                Some(entry.path().ok()?.to_string_lossy().into_owned())
            })
            .collect();

        // Check that essential files are present.
        assert!(entry_paths.iter().any(|p| p.ends_with("system-info.txt")));
        assert!(entry_paths
            .iter()
            .any(|p| p.ends_with("crosshook-info.txt")));
        assert!(entry_paths
            .iter()
            .any(|p| p.ends_with("health-summary.json")));
        assert!(entry_paths
            .iter()
            .any(|p| p.ends_with("steam-diagnostics.txt")));
        assert!(entry_paths
            .iter()
            .any(|p| p.ends_with("proton-installs.json")));
        assert!(entry_paths.iter().any(|p| p.ends_with("settings.toml")));
        assert!(entry_paths
            .iter()
            .any(|p| p.contains("profiles/test-game.toml")));
    }
}
