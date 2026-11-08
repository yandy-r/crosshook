//! Thin IPC adapters over the prefix storage service.

use crosshook_app::services::PrefixStorageHistoryResponse;
use crosshook_core::storage::{PrefixCleanupResult, PrefixCleanupTarget, PrefixStorageScanResult};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn scan_prefix_storage(app: AppState<'_>) -> IpcResult<PrefixStorageScanResult> {
    ipc(app.services().storage.scan().await)
}

#[tauri::command]
pub async fn cleanup_prefix_storage(
    targets: Vec<PrefixCleanupTarget>,
    app: AppState<'_>,
) -> IpcResult<PrefixCleanupResult> {
    ipc(app.services().storage.cleanup(targets).await)
}

#[tauri::command]
pub async fn get_prefix_storage_history(
    app: AppState<'_>,
) -> IpcResult<PrefixStorageHistoryResponse> {
    ipc(app.services().storage.history().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = scan_prefix_storage;
        let _ = cleanup_prefix_storage;
        let _ = get_prefix_storage_history;
    }
}
