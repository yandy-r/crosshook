use super::types::EnrichedProfileHealthReport;
use crosshook_app::services::health::HealthService;
use tauri::State;

/// Returns enriched health for one named profile, with sanitized display paths.
#[tauri::command]
pub async fn get_profile_health(
    name: String,
    service: State<'_, HealthService>,
) -> Result<EnrichedProfileHealthReport, String> {
    service
        .profile_health(name)
        .await
        .map_err(|error| error.to_string())
}
