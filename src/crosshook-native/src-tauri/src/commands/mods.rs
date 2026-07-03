//! Thin IPC surface for the per-profile mod coexistence registry (Forgejo #28).
//! All business logic lives in `crosshook_core::{mods, metadata, launch}`.
//!
//! Error-string contract (frontend keys off prefixes): `metadata_unavailable:`
//! when the DB is down/unusable, `no_game_path:` for the detection scan gate.
//! Everything else is a plain user-facing message.

use std::path::{Path, PathBuf};

use crosshook_core::launch::{analyze_profile_mod_coexistence, LaunchValidationIssue};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::mods::{
    mark_already_registered, scan_game_directory, DetectionScanReport, ProfileModInput,
    ProfileModRecord,
};
use crosshook_core::profile::{GameProfile, ProfileStore};
use serde::Serialize;
use tauri::State;

const METADATA_UNAVAILABLE: &str =
    "metadata_unavailable: the metadata database could not be opened";
const NO_GAME_PATH: &str = "no_game_path: set the game executable path to enable detection";

/// name → (GameProfile, profile_id). Errors are user-facing strings.
async fn resolve_profile(
    profile_name: String,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Result<(GameProfile, String), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let profile = profile_store
            .load(&profile_name)
            .map_err(|error| error.to_string())?;
        let profile_id = match metadata_store.lookup_profile_id(&profile_name) {
            Ok(Some(id)) => id,
            Ok(None) => {
                return Err(
                    "profile has no metadata record yet — save the profile once, then retry"
                        .to_string(),
                )
            }
            Err(error) => return Err(format!("metadata_unavailable: {error}")),
        };
        Ok((profile, profile_id))
    })
    .await
    .map_err(|error| format!("mod registry task failed: {error}"))?
}

#[derive(Serialize)]
pub struct ProfileModsResponse {
    pub available: bool,
    pub mods: Vec<ProfileModRecord>,
}

#[tauri::command]
pub async fn list_profile_mods(
    profile_name: String,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProfileModsResponse, String> {
    let metadata_store = metadata_store.inner().clone();
    if !metadata_store.is_available() {
        return Ok(ProfileModsResponse {
            available: false,
            mods: Vec::new(),
        });
    }
    let profile_store = profile_store.inner().clone();
    let ms = metadata_store.clone();
    let name = profile_name.clone();
    let profile_id = tauri::async_runtime::spawn_blocking(move || {
        profile_store
            .load(&name)
            .map_err(|error| error.to_string())?;
        ms.lookup_profile_id(&name)
            .map_err(|error| format!("metadata_unavailable: {error}"))
    })
    .await
    .map_err(|error| format!("mod registry task failed: {error}"))??;

    let Some(profile_id) = profile_id else {
        return Ok(ProfileModsResponse {
            available: true,
            mods: Vec::new(),
        });
    };

    let mods = tauri::async_runtime::spawn_blocking(move || {
        metadata_store
            .list_profile_mods(&profile_id)
            .map_err(|error| format!("metadata_unavailable: {error}"))
    })
    .await
    .map_err(|error| format!("mod registry task failed: {error}"))??;

    Ok(ProfileModsResponse {
        available: true,
        mods,
    })
}

#[tauri::command]
pub async fn add_profile_mod(
    profile_name: String,
    input: ProfileModInput,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProfileModRecord, String> {
    let metadata_store = metadata_store.inner().clone();
    if !metadata_store.is_available() {
        return Err(METADATA_UNAVAILABLE.to_string());
    }
    let (_, profile_id) = resolve_profile(
        profile_name,
        profile_store.inner().clone(),
        metadata_store.clone(),
    )
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        metadata_store
            .add_profile_mod(&profile_id, &input)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mod registry task failed: {error}"))?
}

#[tauri::command]
pub async fn update_profile_mod(
    profile_name: String,
    mod_id: String,
    input: ProfileModInput,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProfileModRecord, String> {
    let metadata_store = metadata_store.inner().clone();
    if !metadata_store.is_available() {
        return Err(METADATA_UNAVAILABLE.to_string());
    }
    let (_, profile_id) = resolve_profile(
        profile_name,
        profile_store.inner().clone(),
        metadata_store.clone(),
    )
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        metadata_store
            .update_profile_mod(&profile_id, &mod_id, &input)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mod registry task failed: {error}"))?
}

#[tauri::command]
pub async fn remove_profile_mod(
    profile_name: String,
    mod_id: String,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let metadata_store = metadata_store.inner().clone();
    if !metadata_store.is_available() {
        return Err(METADATA_UNAVAILABLE.to_string());
    }
    let (_, profile_id) = resolve_profile(
        profile_name,
        profile_store.inner().clone(),
        metadata_store.clone(),
    )
    .await?;
    let removed = tauri::async_runtime::spawn_blocking(move || {
        metadata_store
            .remove_profile_mod(&profile_id, &mod_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mod registry task failed: {error}"))??;
    if !removed {
        return Err("mod not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn detect_profile_mods(
    profile_name: String,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<DetectionScanReport, String> {
    let profile_store = profile_store.inner().clone();
    let metadata_store = metadata_store.inner().clone();
    let name = profile_name.clone();
    let profile = tauri::async_runtime::spawn_blocking(move || {
        profile_store.load(&name).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mod detection task failed: {error}"))??;

    let effective = profile.effective_profile();
    let game_path = effective.game.executable_path.trim().to_string();
    if game_path.is_empty() {
        return Err(NO_GAME_PATH.to_string());
    }
    let root: PathBuf = match Path::new(&game_path).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => return Err(NO_GAME_PATH.to_string()),
    };

    let scan_root = root.clone();
    let mut report = tauri::async_runtime::spawn_blocking(move || {
        scan_game_directory(&scan_root).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mod detection task failed: {error}"))??;

    // Best-effort `already_registered` enrichment — the scan still works with
    // the DB down (all flags stay false).
    if metadata_store.is_available() {
        let ms = metadata_store.clone();
        let existing = tauri::async_runtime::spawn_blocking(move || {
            let profile_id = ms.lookup_profile_id(&profile_name).ok().flatten()?;
            ms.list_profile_mods(&profile_id).ok()
        })
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
        mark_already_registered(&mut report, &existing, &root);
    }

    Ok(report)
}

#[tauri::command]
pub async fn analyze_mod_coexistence(
    profile_name: String,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<LaunchValidationIssue>, String> {
    let metadata_store = metadata_store.inner().clone();
    if !metadata_store.is_available() {
        return Err(METADATA_UNAVAILABLE.to_string());
    }
    let (profile, profile_id) = resolve_profile(
        profile_name,
        profile_store.inner().clone(),
        metadata_store.clone(),
    )
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        analyze_profile_mod_coexistence(&metadata_store, &profile_id, &profile)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mod analysis task failed: {error}"))?
}
