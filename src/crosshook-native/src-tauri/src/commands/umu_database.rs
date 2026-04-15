use crosshook_core::umu_database;

#[tauri::command]
pub async fn refresh_umu_database() -> Result<umu_database::UmuDatabaseRefreshStatus, String> {
    umu_database::refresh_umu_database()
        .await
        .map_err(|e| e.to_string())
}

/// Lightweight per-title coverage lookup used by the profile Runner selector
/// to surface an advisory warning as soon as a user opts into umu.
///
/// Runs synchronously against the cached CSV index — no network. Returns
/// `Unknown` when the app id is blank, omitted, or no CSV source is reachable.
#[tauri::command]
pub fn check_umu_coverage(app_id: Option<String>) -> umu_database::CsvCoverage {
    let Some(id) = app_id else {
        return umu_database::CsvCoverage::Unknown;
    };
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return umu_database::CsvCoverage::Unknown;
    }
    umu_database::check_coverage(trimmed, Some("steam"))
}
