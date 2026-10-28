//! Thin IPC adapters over the Proton migration service.

use crosshook_app::services::ProtonMigrationService;
use crosshook_app::AppError;
use crosshook_core::profile::migration::{
    ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult, MigrationApplyResult,
    MigrationScanResult,
};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

/// Scan profiles for stale Proton paths, returning display-sanitized suggestions.
#[tauri::command]
pub async fn check_proton_migrations(
    steam_client_install_path: Option<String>,
    service: State<'_, ProtonMigrationService>,
) -> Result<MigrationScanResult, String> {
    service
        .check(steam_client_install_path)
        .await
        .map_err(map_app_error)
}

/// Apply a single migration through the service's shared profile writer.
#[tauri::command]
pub async fn apply_proton_migration(
    request: ApplyMigrationRequest,
    service: State<'_, ProtonMigrationService>,
) -> Result<MigrationApplyResult, String> {
    service.apply(request).await.map_err(map_app_error)
}

/// Apply a batch of migrations through the service's shared profile writer.
#[tauri::command]
pub async fn apply_batch_migration(
    request: BatchMigrationRequest,
    service: State<'_, ProtonMigrationService>,
) -> Result<BatchMigrationResult, String> {
    service.apply_batch(request).await.map_err(map_app_error)
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

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("profile", "profile name is invalid");
        assert_eq!(map_app_error(error), "profile name is invalid");
    }
}
