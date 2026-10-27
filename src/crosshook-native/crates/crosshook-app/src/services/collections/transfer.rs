//! Collection TOML export and import preview.
//!
//! Import stays a non-mutating preview: the caller orchestrates the apply step.

use std::path::PathBuf;

use crosshook_core::profile::{
    export_collection_preset_to_toml, preview_collection_preset_import, CollectionExchangeError,
    CollectionExportResult, CollectionImportPreview,
};

use super::crud::require_available;
use super::CollectionsService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Maps a preset exchange error, keeping its message (the adapter reproduces
/// today's IPC text; `AppError` still redacts the home directory).
fn exchange_error(error: CollectionExchangeError) -> AppError {
    let kind = match &error {
        CollectionExchangeError::Io { .. } | CollectionExchangeError::ProfileStore { .. } => {
            AppErrorKind::Io
        }
        CollectionExchangeError::Metadata { .. } => AppErrorKind::Internal,
        CollectionExchangeError::Toml { .. }
        | CollectionExchangeError::InvalidManifest { .. }
        | CollectionExchangeError::UnsupportedSchemaVersion { .. } => AppErrorKind::Validation {
            field: "collection_preset",
        },
    };
    AppError::new(kind, error.to_string())
}

impl CollectionsService {
    /// Writes the collection preset TOML for `collection_id` to `output_path`.
    pub async fn export_to_toml(
        &self,
        collection_id: String,
        output_path: String,
    ) -> Result<CollectionExportResult, AppError> {
        let collection_id = collection_id.trim().to_owned();
        if collection_id.is_empty() {
            return Err(AppError::validation(
                "collection_id",
                "collection_id must not be empty",
            ));
        }
        if output_path.is_empty() {
            return Err(AppError::validation(
                "output_path",
                "output_path must not be empty",
            ));
        }
        require_available(&self.metadata, "exporting a collection")?;
        let metadata = self.metadata.clone();
        let profiles = self.profiles.clone();
        run_blocking(move || {
            let path = PathBuf::from(output_path);
            export_collection_preset_to_toml(&metadata, &profiles, &collection_id, &path)
                .map_err(exchange_error)
        })
        .await
    }

    /// Parses a collection preset file and classifies each descriptor against
    /// the local profiles without writing anything.
    pub async fn import_from_toml(
        &self,
        path: String,
    ) -> Result<CollectionImportPreview, AppError> {
        if path.is_empty() {
            return Err(AppError::validation("path", "path must not be empty"));
        }
        let profiles = self.profiles.clone();
        run_blocking(move || {
            let path = PathBuf::from(path);
            preview_collection_preset_import(&profiles, &path).map_err(exchange_error)
        })
        .await
    }
}
