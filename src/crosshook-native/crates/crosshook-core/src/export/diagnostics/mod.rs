//! Diagnostic bundle export — collects system info, profiles, logs, and Steam
//! diagnostics into a single `.tar.gz` archive for bug reports.

mod crosshook_info;
mod health;
mod logs;
mod profiles;
mod steam_diagnostics;
mod system_info;

use std::env;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File};
use std::io;
use std::path::PathBuf;

use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use tar::Builder;

use crate::profile::ProfileStore;
use crate::settings::SettingsStore;

use self::crosshook_info::collect_crosshook_info;
use self::health::collect_health_summary;
use self::logs::{collect_app_logs, collect_launch_logs};
use self::profiles::{collect_profiles, collect_settings};
use self::steam_diagnostics::collect_steam_diagnostics;
use self::system_info::collect_system_info;

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

fn append_text<W: io::Write>(
    tar: &mut Builder<W>,
    prefix: &str,
    entry_path: &str,
    content: &str,
) -> Result<(), DiagnosticBundleError> {
    append_bytes(tar, prefix, entry_path, content.as_bytes())
}

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;
    use crate::profile::ProfileStore;
    use crate::settings::SettingsStore;

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
