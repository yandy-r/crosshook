//! Diagnostic bundle export — collects system info, profiles, logs, and Steam
//! diagnostics into a single `.tar.gz` archive for bug reports.

mod crosshook_info;
mod health;
mod launch_logs;
mod logs;
mod metadata_status;
mod profiles;
mod secrets;
mod steam_diagnostics;
mod system_info;
mod url_credentials;

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

use crate::metadata::MetadataStatus;
use crate::profile::ProfileStore;
use crate::settings::SettingsStore;

use self::crosshook_info::collect_crosshook_info;
use self::health::collect_health_summary;
use self::launch_logs::collect_launch_logs;
use self::logs::collect_app_logs;
use self::metadata_status::collect_metadata_status;
use self::profiles::{collect_profiles, collect_settings};
use self::secrets::{settings_secrets, SecretScrubber};
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
    /// Metadata store health. When `Some`, the bundle includes
    /// `metadata-status.json`.
    pub metadata_status: Option<MetadataStatus>,
    /// Owned metadata backup files to list in `metadata-status.json`.
    pub metadata_backup_files: Vec<PathBuf>,
    /// Caller-supplied files written under the bundle's
    /// [`DIAGNOSTIC_BUNDLE_EXTRA_FILES_DIR`] subdirectory. Each name must pass
    /// [`is_valid_bundle_file_name`]; contents are archived verbatim.
    pub extra_files: Vec<DiagnosticBundleFile>,
    /// Recent-files TOML path. Like the settings file, it is never exported as
    /// a profile when it sits in the profiles directory.
    pub recent_files_path: Option<PathBuf>,
}

/// Bundle subdirectory holding [`DiagnosticBundleOptions::extra_files`].
pub const DIAGNOSTIC_BUNDLE_EXTRA_FILES_DIR: &str = "app";

/// A caller-supplied file to include in the diagnostic bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticBundleFile {
    /// Single relative file name (no separators, `..`, or NUL).
    pub name: String,
    /// Raw file contents.
    pub contents: Vec<u8>,
}

/// Returns `true` when `name` is a single relative file name that is safe to
/// place under [`DIAGNOSTIC_BUNDLE_EXTRA_FILES_DIR`]: non-empty, not `.` or
/// `..`, and free of `/`, `\`, and NUL.
pub fn is_valid_bundle_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
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
    if options
        .extra_files
        .iter()
        .any(|file| !is_valid_bundle_file_name(&file.name))
    {
        return Err(DiagnosticBundleError::Archive(
            "invalid extra file name".to_string(),
        ));
    }

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

    // Read settings once so the scrubber and the bundled copy see the same contents.
    let settings_path = settings_store.settings_path();
    let settings_content = fs::read_to_string(&settings_path).ok();

    // Profiles are collected first: the scrubber needs their masked values
    // before any entry that could echo them (health summary, logs) is written.
    let excluded_profile_paths: Vec<PathBuf> = std::iter::once(settings_path)
        .chain(options.recent_files_path.clone())
        .collect();
    let profiles = collect_profiles(profile_store, &excluded_profile_paths, options.redact_paths);

    let scrubber = SecretScrubber::new(
        settings_secrets(settings_content.as_deref().unwrap_or(""))
            .into_iter()
            .chain(profiles.all_secrets().cloned()),
    );
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut bundle = BundleWriter {
        tar: Builder::new(encoder),
        prefix,
        scrubber,
    };

    // -- Collect and add each section --

    let system_info = collect_system_info();
    bundle.append_text("system-info.txt", &system_info)?;

    let crosshook_info = collect_crosshook_info(settings_store, options.redact_paths);
    bundle.append_text("crosshook-info.txt", &crosshook_info)?;

    let health_summary = collect_health_summary(profile_store, &profiles, &bundle.scrubber);
    bundle.append_text("health-summary.json", &health_summary)?;

    if let Some(status) = &options.metadata_status {
        let metadata_text =
            collect_metadata_status(status, &options.metadata_backup_files, options.redact_paths);
        bundle.append_text("metadata-status.json", &metadata_text)?;
    }

    let (steam_diag_text, proton_installs_json, proton_install_count) = collect_steam_diagnostics();
    bundle.append_text("steam-diagnostics.txt", &steam_diag_text)?;
    bundle.append_text("proton-installs.json", &proton_installs_json)?;

    let settings_text = collect_settings(settings_content.as_deref(), options.redact_paths);
    bundle.append_text("settings.toml", &settings_text)?;

    for (name, content) in &profiles.entries {
        bundle.append_text(&format!("profiles/{name}.toml"), content)?;
    }

    let app_logs = collect_app_logs();
    for (filename, data) in &app_logs {
        bundle.append_bytes(&format!("logs/app/{filename}"), data)?;
    }

    let launch_logs = collect_launch_logs();
    for (entry_path, data) in &launch_logs {
        bundle.append_bytes(entry_path, data)?;
    }

    let log_file_count = app_logs.len() + launch_logs.len();

    for file in &options.extra_files {
        let entry_path = format!("{DIAGNOSTIC_BUNDLE_EXTRA_FILES_DIR}/{}", file.name);
        bundle.append_bytes(&entry_path, &file.contents)?;
    }

    // Finalize the archive.
    let encoder = bundle
        .tar
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
            profile_count: profiles.entries.len(),
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

/// Archive writer that scrubs configured secret values from every entry.
struct BundleWriter<W: io::Write> {
    tar: Builder<W>,
    prefix: String,
    scrubber: SecretScrubber,
}

impl<W: io::Write> BundleWriter<W> {
    fn append_text(
        &mut self,
        entry_path: &str,
        content: &str,
    ) -> Result<(), DiagnosticBundleError> {
        self.append_bytes(entry_path, content.as_bytes())
    }

    fn append_bytes(&mut self, entry_path: &str, data: &[u8]) -> Result<(), DiagnosticBundleError> {
        let full_path = format!("{}/{entry_path}", self.prefix);
        let data = self.scrubber.scrub(data);

        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        self.tar
            .append_data(&mut header, &full_path, data.as_ref())
            .map_err(|error| DiagnosticBundleError::Archive(error.to_string()))
    }
}

#[cfg(test)]
mod leak_tests;
#[cfg(test)]
mod tests;
