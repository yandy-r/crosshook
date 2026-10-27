//! Collection launch defaults and the profile defaults merge.

use crosshook_core::metadata::{MetadataStore, MetadataStoreError};
use crosshook_core::profile::{CollectionDefaultsSection, GameProfile};

use super::crud::{require_available, store_error};
use super::CollectionsService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Resolves the defaults layer of `collection_id` with the launch fail-open
/// policy: `Ok(None)` when there is no collection context (absent or blank id),
/// no defaults, or a non-corrupt lookup failure; `Err(Corrupt)` for corrupt
/// defaults so the caller surfaces a loud failure instead of launching with an
/// unexpected profile.
fn resolve_defaults(
    metadata: &MetadataStore,
    collection_id: Option<&str>,
) -> Result<Option<CollectionDefaultsSection>, MetadataStoreError> {
    let Some(cid) = collection_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    match metadata.get_collection_defaults(cid) {
        Ok(defaults) => Ok(defaults),
        Err(MetadataStoreError::Corrupt(msg)) => {
            tracing::warn!(
                collection_id = %cid,
                error = %msg,
                "collection defaults JSON is corrupt; surfacing error to launch entrypoint"
            );
            Err(MetadataStoreError::Corrupt(msg))
        }
        Err(other) => {
            tracing::warn!(
                collection_id = %cid,
                error = %other,
                "failed to load collection defaults; launching with raw profile"
            );
            Ok(None)
        }
    }
}

/// Maps a `resolve_defaults` failure: corrupt defaults keep the legacy
/// `profile_load` text (collection id + JSON parse detail; no paths), so the
/// user can tell which collection to fix. Other errors use the fixed mapping.
fn resolve_error(collection_id: Option<&str>, error: MetadataStoreError) -> AppError {
    match error {
        MetadataStoreError::Corrupt(detail) => {
            let cid = collection_id.map(str::trim).unwrap_or("");
            // Core's detail already names the id; guard if it ever does not.
            let detail = if cid.is_empty() || detail.contains(cid) {
                detail
            } else {
                format!("corrupt collection defaults JSON for {cid}: {detail}")
            };
            AppError::new(
                AppErrorKind::Internal,
                format!("metadata database is corrupt: {detail}"),
            )
        }
        other => store_error(other),
    }
}

impl CollectionsService {
    /// Returns the launch defaults of a collection with unknown keys cleared.
    pub async fn get_defaults(
        &self,
        collection_id: String,
    ) -> Result<Option<CollectionDefaultsSection>, AppError> {
        require_available(&self.metadata, "reading collection defaults")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let mut defaults = metadata
                .get_collection_defaults(&collection_id)
                .map_err(store_error)?;
            if let Some(defaults) = &mut defaults {
                defaults.clear_extra();
            }
            Ok(defaults)
        })
        .await
    }

    /// Stores the launch defaults of a collection (`None` clears them).
    ///
    /// Unknown keys in `defaults` are dropped, then the stored ones are
    /// preserved. The existing-row read fails the call when the stored JSON is
    /// corrupt; clearing with `None` skips that read.
    pub async fn set_defaults(
        &self,
        collection_id: String,
        mut defaults: Option<CollectionDefaultsSection>,
    ) -> Result<(), AppError> {
        require_available(&self.metadata, "writing collection defaults")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            if let Some(defaults) = &mut defaults {
                defaults.clear_extra();
                if let Some(existing) = metadata
                    .get_collection_defaults(&collection_id)
                    .map_err(store_error)?
                {
                    defaults.preserve_extra_from(&existing);
                }
            }
            metadata
                .set_collection_defaults(&collection_id, defaults.as_ref())
                .map_err(store_error)
        })
        .await
    }

    /// Merges the collection launch-defaults layer into a loaded `profile`.
    ///
    /// - `collection_id` absent or blank, no defaults set, or a disabled store:
    ///   `profile` is returned unchanged.
    /// - Corrupt defaults JSON: the error bubbles so the caller can surface it.
    /// - Any other lookup failure (stale id, database glitch): logged, and the
    ///   raw profile is returned so a game launch is never hard-blocked.
    ///
    /// `profile` has already been flattened by `ProfileStore::load`, which bakes
    /// `local_override` into the base layer; see
    /// `GameProfile::effective_profile_with` for the precedence details.
    pub async fn apply_defaults(
        &self,
        profile: GameProfile,
        collection_id: Option<&str>,
    ) -> Result<GameProfile, AppError> {
        let metadata = self.metadata.clone();
        let collection_id = collection_id.map(str::to_owned);
        run_blocking(move || {
            let defaults = resolve_defaults(&metadata, collection_id.as_deref())
                .map_err(|error| resolve_error(collection_id.as_deref(), error))?;
            Ok(Self::resolve(profile, defaults.as_ref()))
        })
        .await
    }

    /// Synchronous variant of the defaults resolution (same policy as
    /// [`Self::apply_defaults`]) for `ProfileWriter::list_summaries`' resolver
    /// closure, which runs at most once and only after the first profile loads.
    pub fn resolve_defaults_sync(
        &self,
        collection_id: Option<&str>,
    ) -> Result<Option<CollectionDefaultsSection>, AppError> {
        resolve_defaults(&self.metadata, collection_id)
            .map_err(|error| resolve_error(collection_id, error))
    }

    /// Pure, store-free merge of `defaults` into `profile`; `None` is the
    /// identity.
    pub fn resolve(
        profile: GameProfile,
        defaults: Option<&CollectionDefaultsSection>,
    ) -> GameProfile {
        match defaults {
            Some(defaults) => profile.effective_profile_with(Some(defaults)),
            None => profile,
        }
    }
}
