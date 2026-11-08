use crosshook_core::offline::{HashVerifyResult, OfflineReadinessReport, TrainerTypeEntry};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn check_offline_readiness(
    name: String,
    app: AppState<'_>,
) -> IpcResult<OfflineReadinessReport> {
    ipc(app.services().offline.check_readiness(name).await)
}

#[tauri::command]
pub async fn batch_offline_readiness(app: AppState<'_>) -> IpcResult<Vec<OfflineReadinessReport>> {
    ipc(app.services().offline.batch_readiness().await)
}

#[tauri::command]
pub async fn verify_trainer_hash(name: String, app: AppState<'_>) -> IpcResult<HashVerifyResult> {
    ipc(app.services().offline.verify_trainer_hash(name).await)
}

#[tauri::command]
pub async fn check_network_status(app: AppState<'_>) -> IpcResult<bool> {
    ipc(app.services().offline.network_status().await)
}

#[tauri::command]
pub async fn get_trainer_type_catalog(app: AppState<'_>) -> IpcResult<Vec<TrainerTypeEntry>> {
    Ok(app.services().offline.trainer_type_catalog())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = check_offline_readiness;
        let _ = batch_offline_readiness;
        let _ = verify_trainer_hash;
        let _ = check_network_status;
        let _ = get_trainer_type_catalog;
    }
}
