//! Version status and snapshot reads.

use std::path::PathBuf;

use crosshook_core::metadata::{
    compute_correlation_status, hash_trainer_file, VersionCorrelationStatus, VersionSnapshotRow,
};
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use crosshook_core::steam::{
    discover_steam_root_candidates, steam_client_install_path_from_compatdata,
};

use super::{VersionCheckResult, VersionService, VersionSnapshotInfo};
use crate::error::AppError;
use crate::services::run_blocking;

pub(super) fn status_from_str(s: &str) -> VersionCorrelationStatus {
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

pub(super) fn row_to_snapshot_info(row: VersionSnapshotRow) -> VersionSnapshotInfo {
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
pub(super) fn locate_manifest_for_app(app_id: &str, steam_client_path: &str) -> Option<PathBuf> {
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

impl VersionService {
    /// Checks the current version correlation status for a named profile.
    pub async fn check_status(&self, name: String) -> Result<VersionCheckResult, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile = profiles.load(&name)?;
            let app_id = profile.steam.app_id.trim().to_string();
            let steam_client_path =
                steam_client_install_path_from_compatdata(&profile.steam.compatdata_path);

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

            let profile_id = metadata
                .lookup_profile_id(&name)
                .map_err(|e| AppError::from_metadata_validation("version", e))?
                .unwrap_or_default();

            let snapshot_row = if !profile_id.is_empty() {
                metadata
                    .lookup_latest_version_snapshot(&profile_id)
                    .map_err(|e| AppError::from_metadata_validation("version", e))?
            } else {
                None
            };

            let status = compute_correlation_status(
                &current_build_id_raw,
                snapshot_row
                    .as_ref()
                    .and_then(|r| r.steam_build_id.as_deref()),
                current_trainer_hash.as_deref(),
                snapshot_row
                    .as_ref()
                    .and_then(|r| r.trainer_file_hash.as_deref()),
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
        })
        .await
    }

    /// Returns the latest recorded version snapshot for a named profile, if any.
    pub async fn snapshot(&self, name: String) -> Result<Option<VersionSnapshotInfo>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = metadata
                .lookup_profile_id(&name)
                .map_err(|e| AppError::from_metadata_validation("version", e))?
                .unwrap_or_default();

            if profile_id.is_empty() {
                return Ok(None);
            }

            let row = metadata
                .lookup_latest_version_snapshot(&profile_id)
                .map_err(|e| AppError::from_metadata_validation("version", e))?;

            Ok(row.map(row_to_snapshot_info))
        })
        .await
    }
}
