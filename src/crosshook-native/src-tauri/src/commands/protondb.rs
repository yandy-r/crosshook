//! Thin IPC adapters over the ProtonDB service. Accepting a suggestion stays
//! on the shared `ProfileWriter`, which owns every profile mutation.

use crosshook_app::services::profiles::ProfileCommit;
use crosshook_app::services::ProtonDbService;
use crosshook_app::{AppError, ProfileWriter};
use crosshook_core::protondb::{
    AcceptSuggestionRequest, AcceptSuggestionResult, ProtonDbLookupResult, ProtonDbSuggestionSet,
};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    service: State<'_, ProtonDbService>,
) -> Result<ProtonDbLookupResult, String> {
    service
        .lookup(app_id, force_refresh.unwrap_or(false))
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    service: State<'_, ProtonDbService>,
) -> Result<ProtonDbSuggestionSet, String> {
    service
        .suggestions(app_id, profile_name, force_refresh.unwrap_or(false))
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn protondb_accept_suggestion(
    request: AcceptSuggestionRequest,
    writer: State<'_, ProfileWriter>,
) -> Result<AcceptSuggestionResult, String> {
    writer
        .accept_protondb_suggestion(request)
        .await
        .map(ProfileCommit::into_output)
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn protondb_dismiss_suggestion(
    profile_name: String,
    app_id: String,
    suggestion_key: String,
    service: State<'_, ProtonDbService>,
) -> Result<(), String> {
    service
        .dismiss_suggestion(profile_name, app_id, suggestion_key)
        .await
        .map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = protondb_lookup;
        let _ = protondb_get_suggestions;
        let _ = protondb_accept_suggestion;
        let _ = protondb_dismiss_suggestion;
    }
}
