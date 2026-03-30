use crosshook_core::metadata::{
    compute_correlation_status, hash_trainer_file, MetadataStore, VersionCorrelationStatus,
    VersionSnapshotRow,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::steam::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

fn map_error(e: impl ToString) -> String {
    e.to_string()
}

/// IPC return type for a version snapshot row, mirroring `VersionSnapshotRow` with
/// `status` parsed to the typed `VersionCorrelationStatus` enum.
#[derive(Debug, Clone, Serialize)]
pub struct VersionSnapshotInfo {
    pub profile_id: String,
    pub steam_app_id: String,
    pub steam_build_id: Option<String>,
    pub trainer_version: Option<String>,
    pub trainer_file_hash: Option<String>,
    pub human_game_ver: Option<String>,
    pub status: VersionCorrelationStatus,
    pub checked_at: String,
}

/// IPC return type for `check_version_status`, assembling the live manifest state
/// with the latest recorded snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct VersionCheckResult {
    pub profile_id: String,
    pub current_build_id: Option<String>,
    pub snapshot: Option<VersionSnapshotInfo>,
    pub status: VersionCorrelationStatus,
    pub update_in_progress: bool,
}

fn status_from_str(s: &str) -> VersionCorrelationStatus {
    match s {
        "matched" => VersionCorrelationStatus::Matched,
        "game_updated" => VersionCorrelationStatus::GameUpdated,
        "trainer_changed" => VersionCorrelationStatus::TrainerChanged,
        "both_changed" => VersionCorrelationStatus::BothChanged,
        "update_in_progress" => VersionCorrelationStatus::UpdateInProgress,
        "untracked" => VersionCorrelationStatus::Untracked,
        _ => VersionCorrelationStatus::Unknown,
    }
}

fn row_to_snapshot_info(row: VersionSnapshotRow) -> VersionSnapshotInfo {
    VersionSnapshotInfo {
        profile_id: row.profile_id,
        steam_app_id: row.steam_app_id,
        steam_build_id: row.steam_build_id,
        trainer_version: row.trainer_version,
        trainer_file_hash: row.trainer_file_hash,
        human_game_ver: row.human_game_ver,
        status: status_from_str(&row.status),
        checked_at: row.checked_at,
    }
}

/// Scan known Steam libraries for `appmanifest_{app_id}.acf` and return the first
/// path found, or `None` if the game is not installed in any discovered library.
fn locate_manifest_for_app(app_id: &str, steam_client_path: &str) -> Option<PathBuf> {
    let mut diagnostics = Vec::new();
    let steam_roots = discover_steam_root_candidates(steam_client_path, &mut diagnostics);
    let libraries = discover_steam_libraries(&steam_roots, &mut diagnostics);

    for entry in &diagnostics {
        tracing::debug!(entry, "version manifest discovery diagnostic");
    }

    for library in &libraries {
        let candidate = library
            .steamapps_path
            .join(format!("appmanifest_{app_id}.acf"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Derive the Steam client install path from a profile's compatdata path.
fn steam_client_install_path_from_profile(profile: &crosshook_core::profile::GameProfile) -> String {
    const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

/// Check the current version correlation status for a named profile.
///
/// Reads the Steam appmanifest for the profile's App ID to get the live build ID
/// and StateFlags, then compares against the latest recorded snapshot. Returns
/// `UpdateInProgress` when Steam is actively updating the game (StateFlags != 4).
#[tauri::command]
pub fn check_version_status(
    name: String,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<VersionCheckResult, String> {
    let profile = profile_store.load(&name).map_err(map_error)?;
    let app_id = profile.steam.app_id.trim().to_string();
    let steam_client_path = steam_client_install_path_from_profile(&profile);

    let (current_build_id_raw, state_flags): (String, Option<u32>) = if !app_id.is_empty() {
        locate_manifest_for_app(&app_id, &steam_client_path)
            .and_then(|path| parse_manifest_full(&path).ok())
            .map(|data| (data.build_id, data.state_flags))
            .unwrap_or_default()
    } else {
        Default::default()
    };

    let current_build_id = if current_build_id_raw.is_empty() {
        None
    } else {
        Some(current_build_id_raw.clone())
    };
    let trainer_path = profile.trainer.path.trim().to_string();
    let current_trainer_hash = if trainer_path.is_empty() {
        None
    } else {
        hash_trainer_file(std::path::Path::new(&trainer_path))
    };

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(map_error)?
        .unwrap_or_default();

    let snapshot_row = if !profile_id.is_empty() {
        metadata_store
            .lookup_latest_version_snapshot(&profile_id)
            .map_err(map_error)?
    } else {
        None
    };

    let status = compute_correlation_status(
        &current_build_id_raw,
        snapshot_row.as_ref().and_then(|r| r.steam_build_id.as_deref()),
        current_trainer_hash.as_deref(),
        snapshot_row.as_ref().and_then(|r| r.trainer_file_hash.as_deref()),
        state_flags,
    );

    let update_in_progress = matches!(status, VersionCorrelationStatus::UpdateInProgress);
    let snapshot = snapshot_row.map(row_to_snapshot_info);

    Ok(VersionCheckResult {
        profile_id,
        current_build_id,
        snapshot,
        status,
        update_in_progress,
    })
}

/// Return the latest recorded version snapshot for a named profile, if any.
#[tauri::command]
pub fn get_version_snapshot(
    name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Option<VersionSnapshotInfo>, String> {
    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(map_error)?
        .unwrap_or_default();

    if profile_id.is_empty() {
        return Ok(None);
    }

    let row = metadata_store
        .lookup_latest_version_snapshot(&profile_id)
        .map_err(map_error)?;

    Ok(row.map(row_to_snapshot_info))
}

/// Record a manual trainer version hint for a named profile.
///
/// Upserts a new snapshot row with `status = "untracked"` and the provided
/// version string. The `steam_app_id` is preserved from the latest existing
/// snapshot, or left empty if no snapshot has been recorded yet.
#[tauri::command]
pub fn set_trainer_version(
    name: String,
    version: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(map_error)?
        .ok_or_else(|| format!("profile '{name}' is not registered in the metadata store"))?;

    let steam_app_id = metadata_store
        .lookup_latest_version_snapshot(&profile_id)
        .ok()
        .flatten()
        .map(|row| row.steam_app_id)
        .unwrap_or_default();

    metadata_store
        .upsert_version_snapshot(
            &profile_id,
            &steam_app_id,
            None,
            Some(&version),
            None,
            None,
            "untracked",
        )
        .map_err(map_error)
}

/// Mark the latest version change for a named profile as acknowledged.
///
/// Sets the latest snapshot's status to `"matched"`, indicating the user has
/// verified the trainer still works with the updated game version.
#[tauri::command]
pub fn acknowledge_version_change(
    name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(map_error)?
        .ok_or_else(|| format!("profile '{name}' is not registered in the metadata store"))?;

    metadata_store
        .acknowledge_version_change(&profile_id)
        .map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = check_version_status
            as fn(
                String,
                State<'_, MetadataStore>,
                State<'_, ProfileStore>,
            ) -> Result<VersionCheckResult, String>;
        let _ = get_version_snapshot
            as fn(String, State<'_, MetadataStore>) -> Result<Option<VersionSnapshotInfo>, String>;
        let _ = set_trainer_version
            as fn(String, String, State<'_, MetadataStore>) -> Result<(), String>;
        let _ = acknowledge_version_change
            as fn(String, State<'_, MetadataStore>) -> Result<(), String>;
    }
}
