//! Diagnostics service: user-requested `.tar.gz` diagnostic bundle export.

use std::path::PathBuf;

use crosshook_core::export::{
    export_diagnostic_bundle, is_valid_bundle_file_name, DiagnosticBundleFile,
    DiagnosticBundleOptions, DiagnosticBundleSummary,
};
use crosshook_core::metadata::{MetadataStatus, MetadataStore};
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::{RecentFilesStore, SettingsStore};

use super::run_blocking;
use crate::presenter::{sanitize_display_path_with_home, sanitize_message};
use crate::AppError;

/// Extra file a frontend contributes to the bundle (native UI logs, renderer
/// info, crash logs). Archived under the bundle's `app/` subdirectory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticAttachment {
    /// Single relative file name: non-empty, not `.`/`..`, and free of `/`,
    /// `\`, and NUL.
    pub name: String,
    /// Raw contents. UTF-8 contents are home-path redacted when
    /// [`DiagnosticsRequest::redact_paths`] is set; binary data is kept as is.
    pub contents: Vec<u8>,
}

/// Diagnostic export parameters.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticsRequest {
    /// Redact the home directory from collected text.
    pub redact_paths: bool,
    /// Output directory; defaults to the system temp directory.
    pub output_dir: Option<PathBuf>,
    /// Extra files to include under `app/`.
    pub attachments: Vec<DiagnosticAttachment>,
}

/// Result of a successful export.
#[derive(Debug, Clone)]
pub struct DiagnosticsExport {
    /// Absolute archive path.
    pub archive_path: PathBuf,
    /// Archive path with the home directory redacted, for display.
    pub display_path: String,
    /// Summary of collected content.
    pub summary: DiagnosticBundleSummary,
}

/// Diagnostics service. Cheap to clone.
#[derive(Clone)]
pub struct DiagnosticsService {
    profile_store: ProfileStore,
    settings_store: SettingsStore,
    recent_files_store: RecentFilesStore,
    metadata_store: MetadataStore,
    /// Home directory redacted from [`DiagnosticsExport::display_path`];
    /// read from `HOME` at construction.
    display_home: Option<PathBuf>,
}

impl DiagnosticsService {
    /// Creates a service over the given stores.
    pub fn new(
        profile_store: ProfileStore,
        settings_store: SettingsStore,
        recent_files_store: RecentFilesStore,
        metadata_store: MetadataStore,
    ) -> Self {
        Self {
            profile_store,
            settings_store,
            recent_files_store,
            metadata_store,
            display_home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }

    /// Overrides the home directory redacted from display paths (tests).
    #[cfg(test)]
    pub(crate) fn with_display_home(mut self, home: PathBuf) -> Self {
        self.display_home = Some(home);
        self
    }

    /// Metadata store health for the UI degraded-state banner. In-memory
    /// read; the JSON shape is part of the IPC contract: `{"state":"ok"}`,
    /// `{"state":"newer_schema","found":N,"supported":N}`,
    /// `{"state":"disabled","reason":"..."}`.
    pub fn metadata_status(&self) -> MetadataStatus {
        self.metadata_store.status()
    }

    /// Writes a diagnostic bundle. Invalid attachment names fail with a
    /// validation error before anything is written.
    pub async fn export(&self, request: DiagnosticsRequest) -> Result<DiagnosticsExport, AppError> {
        if request
            .attachments
            .iter()
            .any(|attachment| !is_valid_bundle_file_name(&attachment.name))
        {
            return Err(AppError::validation(
                "attachments",
                "attachment name must be a single relative file name",
            ));
        }
        let service = self.clone();
        run_blocking(move || {
            let redact = request.redact_paths;
            let extra_files = request
                .attachments
                .into_iter()
                .map(|attachment| DiagnosticBundleFile {
                    name: attachment.name,
                    contents: if redact {
                        redact_contents(attachment.contents)
                    } else {
                        attachment.contents
                    },
                })
                .collect();
            let options = DiagnosticBundleOptions {
                redact_paths: redact,
                output_dir: request.output_dir,
                metadata_status: Some(service.metadata_store.status()),
                metadata_backup_files: service.metadata_store.backup_files(),
                extra_files,
                recent_files_path: Some(service.recent_files_store.path.clone()),
            };
            let result = export_diagnostic_bundle(
                &service.profile_store,
                &service.settings_store,
                &options,
            )?;
            let archive_path = PathBuf::from(result.archive_path);
            Ok(DiagnosticsExport {
                display_path: sanitize_display_path_with_home(
                    &archive_path,
                    service.display_home.as_deref(),
                ),
                archive_path,
                summary: result.summary,
            })
        })
        .await
    }
}

/// Redacts home paths in UTF-8 `contents`; non-UTF-8 data is returned as is.
fn redact_contents(contents: Vec<u8>) -> Vec<u8> {
    match String::from_utf8(contents) {
        Ok(text) => sanitize_message(&text).into_bytes(),
        Err(error) => error.into_bytes(),
    }
}

#[cfg(test)]
mod tests;
