//! Health Tauri commands: thin delegates to `HealthService`.

use crosshook_app::services::health::{
    CachedHealthSnapshot, CachedOfflineReadinessSnapshot, EnrichedHealthSummary,
    EnrichedProfileHealthReport,
};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

/// Returns enriched health results for all profiles, with sanitized display paths.
#[tauri::command]
pub async fn batch_validate_profiles(app: AppState<'_>) -> IpcResult<EnrichedHealthSummary> {
    ipc(app.services().health.batch_summary().await)
}

#[tauri::command]
pub async fn get_profile_health(
    name: String,
    app: AppState<'_>,
) -> IpcResult<EnrichedProfileHealthReport> {
    ipc(app.services().health.profile_health(name).await)
}

/// Returns the last batch's cached health snapshots for instant badge display.
#[tauri::command]
pub async fn get_cached_health_snapshots(
    app: AppState<'_>,
) -> IpcResult<Vec<CachedHealthSnapshot>> {
    ipc(app.services().health.cached_health_snapshots().await)
}

/// Returns cached offline-readiness snapshots for existing profiles.
#[tauri::command]
pub async fn get_cached_offline_readiness_snapshots(
    app: AppState<'_>,
) -> IpcResult<Vec<CachedOfflineReadinessSnapshot>> {
    ipc(app
        .services()
        .health
        .cached_offline_readiness_snapshots()
        .await)
}
