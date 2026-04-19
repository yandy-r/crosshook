use std::path::PathBuf;

use crosshook_core::launch::{
    collect_trainer_hash_launch_warnings, LaunchRequest, LaunchValidationIssue, ValidationError,
    METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::offline::readiness::MIN_OFFLINE_READINESS_SCORE;
use crosshook_core::profile::ProfileStore;
use crosshook_core::storage::{check_low_disk_warning, DEFAULT_LOW_DISK_WARNING_MB};

/// Non-blocking offline readiness advisory when the profile has a trainer configured.
pub(super) async fn collect_offline_launch_warnings(
    request: &LaunchRequest,
    profile_name: Option<String>,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Vec<LaunchValidationIssue> {
    let mut warnings = collect_low_disk_warning(request).await;
    let Some(name) = profile_name.filter(|n| !n.trim().is_empty()) else {
        return warnings;
    };
    if !metadata_store.is_available() {
        return warnings;
    }
    let ps = profile_store;
    let ms = metadata_store;
    let mut offline_warnings = tauri::async_runtime::spawn_blocking(move || {
        let profile = match ps.load(&name) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        if profile.effective_profile().trainer.path.trim().is_empty() {
            return Vec::new();
        }
        let profile_id = match ms.lookup_profile_id(&name) {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return Vec::new(),
        };
        let report = match ms.check_offline_readiness_for_profile(&name, &profile_id, &profile) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        if report.score >= MIN_OFFLINE_READINESS_SCORE {
            return Vec::new();
        }
        vec![ValidationError::OfflineReadinessInsufficient {
            score: report.score,
            reasons: report.blocking_reasons.clone(),
        }
        .issue()]
    })
    .await
    .unwrap_or_default();
    warnings.append(&mut offline_warnings);
    warnings
}

/// SHA-256 baseline / community digest advisory (non-blocking).
pub(super) async fn collect_trainer_hash_launch_warnings_ipc(
    profile_name: Option<String>,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Vec<LaunchValidationIssue> {
    let Some(name) = profile_name.filter(|n| !n.trim().is_empty()) else {
        return Vec::new();
    };
    if !metadata_store.is_available() {
        return Vec::new();
    }
    let ps = profile_store;
    let ms = metadata_store;
    tauri::async_runtime::spawn_blocking(move || {
        let profile = match ps.load(&name) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        let profile_id = match ms.lookup_profile_id(&name) {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return Vec::new(),
        };
        collect_trainer_hash_launch_warnings(&ms, &profile_id, &profile)
    })
    .await
    .unwrap_or_default()
}

async fn collect_low_disk_warning(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let raw_prefix_path = if request.resolved_method() == METHOD_STEAM_APPLAUNCH {
        request.steam.compatdata_path.trim()
    } else {
        request.runtime.prefix_path.trim()
    };
    if raw_prefix_path.is_empty() {
        return Vec::new();
    }

    let prefix_path = PathBuf::from(raw_prefix_path);
    let check_result = tauri::async_runtime::spawn_blocking(move || {
        check_low_disk_warning(&prefix_path, DEFAULT_LOW_DISK_WARNING_MB)
    })
    .await;

    let warning = match check_result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            tracing::warn!(path = raw_prefix_path, %error, "low-disk check failed");
            None
        }
        Err(error) => {
            tracing::warn!(path = raw_prefix_path, %error, "low-disk check task failed");
            None
        }
    };

    match warning {
        Some(value) => vec![ValidationError::LowDiskSpaceAdvisory {
            available_mb: value.available_bytes / (1024 * 1024),
            threshold_mb: value.threshold_bytes / (1024 * 1024),
            mount_path: value.mount_path,
        }
        .issue()],
        None => Vec::new(),
    }
}
