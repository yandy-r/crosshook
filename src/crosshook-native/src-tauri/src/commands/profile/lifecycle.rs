use std::path::PathBuf;

use crosshook_app::services::profiles::{ProfileCommit, ProfileSummary};
use crosshook_app::ProfileWriter;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{DuplicateProfileResult, GameProfile};
use tauri::State;

use super::shared::{apply_collection_defaults, resolve_collection_defaults};

#[tauri::command]
pub async fn profile_list(writer: State<'_, ProfileWriter>) -> Result<Vec<String>, String> {
    writer.list().await.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_load(
    name: String,
    collection_id: Option<String>,
    writer: State<'_, ProfileWriter>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    let profile = writer.load(name).await.map_err(|error| error.to_string())?;

    // When a collection context is provided, merge the collection's defaults
    // into the profile via `effective_profile_with`. The returned profile still
    // reflects the machine-specific `local_override` layer that `ProfileStore::load`
    // baked into layer 1, so collection defaults can never clobber portable paths.
    let mut profile =
        apply_collection_defaults(profile, metadata_store.inner(), collection_id.as_deref())
            .map_err(|e| e.to_string())?;
    profile.clear_extra();
    Ok(profile)
}

#[tauri::command]
pub async fn profile_list_summaries(
    collection_id: Option<String>,
    writer: State<'_, ProfileWriter>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<ProfileSummary>, String> {
    // Collection composition stays in the shell until M1-35 (D7).
    let metadata = metadata_store.inner().clone();
    writer
        .list_summaries(move || {
            resolve_collection_defaults(&metadata, collection_id.as_deref()).map_err(Into::into)
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_save(
    name: String,
    data: GameProfile,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .save(name, data)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_delete(name: String, writer: State<'_, ProfileWriter>) -> Result<(), String> {
    writer
        .delete(name)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

/// Duplicates an existing profile under a unique copy name.
///
/// The returned [`DuplicateProfileResult`] is serialized to the frontend where it
/// drives profile list refresh and auto-selection of the copy.
///
/// # Frontend invocation
/// ```ts
/// const result = await invoke<DuplicateProfileResult>('profile_duplicate', { name });
/// ```
///
/// # Errors
/// Returns a stringified error when the source profile does not exist or if the
/// generated copy name cannot pass filesystem validation.
#[tauri::command]
pub async fn profile_duplicate(
    name: String,
    writer: State<'_, ProfileWriter>,
) -> Result<DuplicateProfileResult, String> {
    writer
        .duplicate(name)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

/// Renames a profile; returns whether exported launcher files were removed.
#[tauri::command]
pub async fn profile_rename(
    old_name: String,
    new_name: String,
    writer: State<'_, ProfileWriter>,
) -> Result<bool, String> {
    writer
        .rename(old_name, new_name)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_import_legacy(
    path: String,
    writer: State<'_, ProfileWriter>,
) -> Result<GameProfile, String> {
    writer
        .import_legacy(PathBuf::from(path))
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

/// Serializes the provided in-memory profile to a shareable TOML string
/// with comment headers indicating the save location.
#[tauri::command]
pub async fn profile_export_toml(
    name: String,
    data: GameProfile,
    include_hooks: Option<bool>,
    writer: State<'_, ProfileWriter>,
) -> Result<String, String> {
    writer
        .export_toml(name, data, include_hooks.unwrap_or(false))
        .await
        .map_err(|error| error.to_string())
}
