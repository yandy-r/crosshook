//! Thin IPC adapters over the Proton migration service.

use crosshook_core::profile::migration::{
    ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult, MigrationApplyResult,
    MigrationScanResult,
};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn check_proton_migrations(
    steam_client_install_path: Option<String>,
    app: AppState<'_>,
) -> IpcResult<MigrationScanResult> {
    ipc(app
        .services()
        .proton_migration
        .check(steam_client_install_path)
        .await)
}

#[tauri::command]
pub async fn apply_proton_migration(
    request: ApplyMigrationRequest,
    app: AppState<'_>,
) -> IpcResult<MigrationApplyResult> {
    ipc(app.services().proton_migration.apply(request).await)
}

#[tauri::command]
pub async fn apply_batch_migration(
    request: BatchMigrationRequest,
    app: AppState<'_>,
) -> IpcResult<BatchMigrationResult> {
    ipc(app.services().proton_migration.apply_batch(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = check_proton_migrations;
        let _ = apply_proton_migration;
        let _ = apply_batch_migration;
    }
}
