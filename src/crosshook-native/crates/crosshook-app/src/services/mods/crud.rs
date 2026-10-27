//! Registry reads and writes for the per-profile mod list.

use crosshook_core::metadata::{MetadataStore, MetadataStoreError};
use crosshook_core::mods::{ProfileModInput, ProfileModRecord};
use crosshook_core::profile::{GameProfile, ProfileStore};

use super::{ModsService, ProfileModsResponse};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Message the Tauri adapter maps to the legacy `metadata_unavailable:` string.
const METADATA_UNAVAILABLE: &str =
    "metadata_unavailable: the metadata database could not be opened";
const MISSING_IDENTITY: &str =
    "profile has no metadata record yet — save the profile once, then retry";

/// Error for a write or analysis attempted while the metadata DB is down.
pub(super) fn unavailable() -> AppError {
    AppError::new(AppErrorKind::Unavailable, METADATA_UNAVAILABLE)
}

/// Maps a store error from the mods registry (validation texts are kept).
pub(super) fn store_error(error: MetadataStoreError) -> AppError {
    AppError::from_metadata_validation("mod", error)
}

/// Looks up the metadata id of `name`; lookup failures keep the legacy
/// `metadata_unavailable: {error}` text.
fn lookup_id(metadata: &MetadataStore, name: &str) -> Result<Option<String>, AppError> {
    metadata.lookup_profile_id(name).map_err(|error| {
        AppError::new(
            AppErrorKind::Unavailable,
            format!("metadata_unavailable: {error}"),
        )
    })
}

/// Resolves a profile name to its loaded profile and metadata id. A profile
/// without a metadata row is `NotFound` so the user can save it once and retry.
pub(super) fn resolve_profile(
    profiles: &ProfileStore,
    metadata: &MetadataStore,
    name: &str,
) -> Result<(GameProfile, String), AppError> {
    let profile = profiles.load(name)?;
    match lookup_id(metadata, name)? {
        Some(id) => Ok((profile, id)),
        None => Err(AppError::new(AppErrorKind::NotFound, MISSING_IDENTITY)),
    }
}

impl ModsService {
    /// Lists the registered mods of `profile_name`. With the metadata database
    /// down this is `{ available: false, mods: [] }`; a profile that has no
    /// metadata row yet has an empty registry.
    pub async fn list(&self, profile_name: String) -> Result<ProfileModsResponse, AppError> {
        if !self.metadata.is_available() {
            return Ok(ProfileModsResponse {
                available: false,
                mods: Vec::new(),
            });
        }
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            profiles.load(&profile_name)?;
            let mods = match lookup_id(&metadata, &profile_name)? {
                Some(profile_id) => metadata
                    .list_profile_mods(&profile_id)
                    .map_err(store_error)?,
                None => Vec::new(),
            };
            Ok(ProfileModsResponse {
                available: true,
                mods,
            })
        })
        .await
    }

    /// Registers a mod for `profile_name`.
    pub async fn add(
        &self,
        profile_name: String,
        input: ProfileModInput,
    ) -> Result<ProfileModRecord, AppError> {
        if !self.metadata.is_available() {
            return Err(unavailable());
        }
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let (_, profile_id) = resolve_profile(&profiles, &metadata, &profile_name)?;
            metadata
                .add_profile_mod(&profile_id, &input)
                .map_err(store_error)
        })
        .await
    }

    /// Replaces the registered mod `mod_id` of `profile_name`; a `mod_id`
    /// belonging to another profile is `mod not found`.
    pub async fn update(
        &self,
        profile_name: String,
        mod_id: String,
        input: ProfileModInput,
    ) -> Result<ProfileModRecord, AppError> {
        if !self.metadata.is_available() {
            return Err(unavailable());
        }
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let (_, profile_id) = resolve_profile(&profiles, &metadata, &profile_name)?;
            metadata
                .update_profile_mod(&profile_id, &mod_id, &input)
                .map_err(store_error)
        })
        .await
    }

    /// Removes the registered mod `mod_id` of `profile_name` (registry only;
    /// no files are touched).
    pub async fn remove(&self, profile_name: String, mod_id: String) -> Result<(), AppError> {
        if !self.metadata.is_available() {
            return Err(unavailable());
        }
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let (_, profile_id) = resolve_profile(&profiles, &metadata, &profile_name)?;
            let removed = metadata
                .remove_profile_mod(&profile_id, &mod_id)
                .map_err(store_error)?;
            if removed {
                Ok(())
            } else {
                // Legacy IPC text for a stale remove is the bare `mod not found`
                // (no store-error prefix); `useProfileMods` shows it verbatim.
                Err(AppError::new(AppErrorKind::NotFound, "mod not found"))
            }
        })
        .await
    }
}
