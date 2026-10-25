use crosshook_app::services::health::{
    CachedHealthSnapshot, CachedOfflineReadinessSnapshot, HealthService,
};
use tauri::State;

/// Returns the last batch's cached health snapshots for instant badge display.
#[tauri::command]
pub async fn get_cached_health_snapshots(
    service: State<'_, HealthService>,
) -> Result<Vec<CachedHealthSnapshot>, String> {
    service
        .cached_health_snapshots()
        .await
        .map_err(|error| error.to_string())
}

/// Returns cached offline-readiness snapshots for existing profiles.
#[tauri::command]
pub async fn get_cached_offline_readiness_snapshots(
    service: State<'_, HealthService>,
) -> Result<Vec<CachedOfflineReadinessSnapshot>, String> {
    service
        .cached_offline_readiness_snapshots()
        .await
        .map_err(|error| error.to_string())
}
