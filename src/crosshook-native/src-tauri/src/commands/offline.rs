use crosshook_app::services::OfflineService;
use crosshook_core::offline::{HashVerifyResult, OfflineReadinessReport, TrainerTypeEntry};
use tauri::State;

#[tauri::command]
pub async fn check_offline_readiness(
    name: String,
    service: State<'_, OfflineService>,
) -> Result<OfflineReadinessReport, String> {
    service
        .check_readiness(name)
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn batch_offline_readiness(
    service: State<'_, OfflineService>,
) -> Result<Vec<OfflineReadinessReport>, String> {
    service
        .batch_readiness()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn verify_trainer_hash(
    name: String,
    service: State<'_, OfflineService>,
) -> Result<HashVerifyResult, String> {
    service
        .verify_trainer_hash(name)
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn check_network_status(service: State<'_, OfflineService>) -> Result<bool, String> {
    service
        .network_status()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub fn get_trainer_type_catalog(
    service: State<'_, OfflineService>,
) -> Result<Vec<TrainerTypeEntry>, String> {
    Ok(service.trainer_type_catalog())
}
