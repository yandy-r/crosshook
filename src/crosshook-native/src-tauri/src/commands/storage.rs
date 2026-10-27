//! Thin IPC adapters over the prefix storage service (Forgejo #28).
//!
//! All scan, cleanup and history logic lives in
//! `crosshook_app::services::storage`; the adapter only extracts the managed
//! service, awaits the call and maps the sanitized [`AppError`] message to the
//! legacy IPC error string (`prefix storage {scan|cleanup|history} task
//! failed: …` on task failures, built inside the service).

use crosshook_app::services::storage::{PrefixStorageHistoryResponse, StorageService};
use crosshook_core::storage::{PrefixCleanupResult, PrefixCleanupTarget, PrefixStorageScanResult};
use tauri::State;

fn map_app_error(error: crosshook_app::AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn scan_prefix_storage(
    service: State<'_, StorageService>,
) -> Result<PrefixStorageScanResult, String> {
    service.scan().await.map_err(map_app_error)
}

#[tauri::command]
pub async fn cleanup_prefix_storage(
    targets: Vec<PrefixCleanupTarget>,
    service: State<'_, StorageService>,
) -> Result<PrefixCleanupResult, String> {
    service.cleanup(targets).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn get_prefix_storage_history(
    service: State<'_, StorageService>,
) -> Result<PrefixStorageHistoryResponse, String> {
    service.history().await.map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = scan_prefix_storage;
        let _ = cleanup_prefix_storage;
        let _ = get_prefix_storage_history;
    }
}
