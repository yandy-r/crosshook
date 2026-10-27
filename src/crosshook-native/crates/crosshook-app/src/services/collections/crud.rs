//! Collection CRUD and membership.
//!
//! Writes and honest reads return `Unavailable` before touching the store when
//! the metadata database is down; `list` and `collections_for_profile` keep
//! their historical `Ok(vec![])` on a disabled store.

use crosshook_core::metadata::{CollectionRow, MetadataStore, MetadataStoreError};

use super::CollectionsService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Fails with `Unavailable` when the metadata database cannot serve `operation`.
pub(super) fn require_available(metadata: &MetadataStore, operation: &str) -> Result<(), AppError> {
    if metadata.is_available() {
        Ok(())
    } else {
        Err(AppError::new(
            AppErrorKind::Unavailable,
            format!("{operation} is unavailable — metadata store is not accessible"),
        ))
    }
}

/// Maps a collections-store error (validation texts such as
/// `collection not found: …` are kept).
pub(super) fn store_error(error: MetadataStoreError) -> AppError {
    AppError::from_metadata_validation("collection", error)
}

impl CollectionsService {
    /// Lists all collections; an unavailable store yields an empty list.
    pub async fn list(&self) -> Result<Vec<CollectionRow>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || metadata.list_collections().map_err(store_error)).await
    }

    /// Creates a collection and returns its id.
    pub async fn create(&self, name: String) -> Result<String, AppError> {
        require_available(&self.metadata, "creating a collection")?;
        let metadata = self.metadata.clone();
        run_blocking(move || metadata.create_collection(&name).map_err(store_error)).await
    }

    /// Deletes a collection (memberships and defaults cascade). An unknown id
    /// is a silent success.
    pub async fn delete(&self, collection_id: String) -> Result<(), AppError> {
        require_available(&self.metadata, "deleting a collection")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .delete_collection(&collection_id)
                .map_err(store_error)
        })
        .await
    }

    /// Adds a registered profile to a collection (idempotent).
    pub async fn add_profile(
        &self,
        collection_id: String,
        profile_name: String,
    ) -> Result<(), AppError> {
        require_available(&self.metadata, "adding a profile to a collection")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .add_profile_to_collection(&collection_id, &profile_name)
                .map_err(store_error)
        })
        .await
    }

    /// Removes a profile from a collection; absent membership is a no-op.
    pub async fn remove_profile(
        &self,
        collection_id: String,
        profile_name: String,
    ) -> Result<(), AppError> {
        require_available(&self.metadata, "removing a profile from a collection")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .remove_profile_from_collection(&collection_id, &profile_name)
                .map_err(store_error)
        })
        .await
    }

    /// Lists the profile names that belong to a collection.
    pub async fn list_profiles(&self, collection_id: String) -> Result<Vec<String>, AppError> {
        require_available(&self.metadata, "listing collection profiles")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .list_profiles_in_collection(&collection_id)
                .map_err(store_error)
        })
        .await
    }

    /// Renames a collection.
    pub async fn rename(&self, collection_id: String, new_name: String) -> Result<(), AppError> {
        require_available(&self.metadata, "renaming a collection")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .rename_collection(&collection_id, &new_name)
                .map_err(store_error)
        })
        .await
    }

    /// Sets or clears the description of a collection.
    pub async fn update_description(
        &self,
        collection_id: String,
        description: Option<String>,
    ) -> Result<(), AppError> {
        require_available(&self.metadata, "updating a collection description")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .update_collection_description(&collection_id, description.as_deref())
                .map_err(store_error)
        })
        .await
    }

    /// Lists the collections a profile belongs to; an unavailable store yields
    /// an empty list.
    pub async fn collections_for_profile(
        &self,
        profile_name: String,
    ) -> Result<Vec<CollectionRow>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            metadata
                .collections_for_profile(&profile_name)
                .map_err(store_error)
        })
        .await
    }
}
