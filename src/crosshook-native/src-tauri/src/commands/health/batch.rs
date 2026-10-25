use crosshook_app::services::health::{EnrichedHealthSummary, HealthService};
use tauri::State;

/// Returns enriched health results for all profiles, with sanitized display paths.
#[tauri::command]
pub async fn batch_validate_profiles(
    service: State<'_, HealthService>,
) -> Result<EnrichedHealthSummary, String> {
    service
        .batch_summary()
        .await
        .map_err(|error| error.to_string())
}
