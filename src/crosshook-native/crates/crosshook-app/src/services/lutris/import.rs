//! Lutris preview and import operations.

use std::path::PathBuf;

use crosshook_core::profile::lutris_import::preview_lutris_import;
use crosshook_core::profile::{LutrisImportEntry, LutrisImportPreview, LutrisImportResult};

use super::LutrisService;
use crate::services::profiles::ProfileCommit;
use crate::services::run_blocking;
use crate::AppError;

impl LutrisService {
    /// Scans the Lutris library at `directory` (or the discovered default
    /// root) and previews importable entries without writing anything.
    /// A missing library is not an error: the preview is empty and carries a
    /// diagnostic.
    ///
    /// # Errors
    /// Pipeline failures only (task panic or cancellation).
    pub async fn prepare_import(
        &self,
        directory: Option<PathBuf>,
    ) -> Result<LutrisImportPreview, AppError> {
        let profiles = self.profiles.clone();
        run_blocking(move || {
            preview_lutris_import(directory, Some(&profiles)).map_err(AppError::from)
        })
        .await
    }

    /// Imports the selected `entries` through the shared profile writer.
    /// Per-entry failures and skipped entries are reported in the result, not
    /// as errors; `Imported` is published with the written names.
    ///
    /// # Errors
    /// Writer pipeline failures (cancellation).
    pub async fn import_profiles(
        &self,
        entries: Vec<LutrisImportEntry>,
    ) -> Result<LutrisImportResult, AppError> {
        self.writer
            .import_lutris(entries)
            .await
            .map(ProfileCommit::into_output)
    }
}
