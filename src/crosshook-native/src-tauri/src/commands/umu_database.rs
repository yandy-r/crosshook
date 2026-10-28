//! Thin IPC adapters over the umu database service.

use crosshook_app::services::UmuService;
use crosshook_app::AppError;
use crosshook_core::umu_database;
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn refresh_umu_database(
    service: State<'_, UmuService>,
) -> Result<umu_database::UmuDatabaseRefreshStatus, String> {
    service.refresh().await.map_err(map_app_error)
}

/// Lightweight cached coverage lookup used by the profile Runner selector.
/// Blank or omitted app ids return Unknown without a network request.
#[tauri::command]
pub async fn check_umu_coverage(
    app_id: Option<String>,
    service: State<'_, UmuService>,
) -> Result<umu_database::CsvCoverage, String> {
    service.coverage(app_id).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn clear_umu_gameid_lookup_cache(
    service: State<'_, UmuService>,
) -> Result<usize, String> {
    service.clear_gameid_cache().await.map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = refresh_umu_database;
        let _ = check_umu_coverage;
        let _ = clear_umu_gameid_lookup_cache;
    }

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("app_id", "app id is invalid");
        assert_eq!(map_app_error(error), "app id is invalid");
    }
}
