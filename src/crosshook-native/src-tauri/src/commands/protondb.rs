//! Thin IPC adapters over the ProtonDB service. Accepting a suggestion stays
//! on the shared `ProfileWriter`, which owns every profile mutation.

use crosshook_core::protondb::{
    AcceptSuggestionRequest, AcceptSuggestionResult, ProtonDbLookupResult, ProtonDbSuggestionSet,
};

use super::{AppState, IpcResult};
use crate::error_map::{commit, ipc};

#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    app: AppState<'_>,
) -> IpcResult<ProtonDbLookupResult> {
    ipc(app
        .services()
        .protondb
        .lookup(app_id, force_refresh.unwrap_or(false))
        .await)
}

#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    app: AppState<'_>,
) -> IpcResult<ProtonDbSuggestionSet> {
    ipc(app
        .services()
        .protondb
        .suggestions(app_id, profile_name, force_refresh.unwrap_or(false))
        .await)
}

#[tauri::command]
pub async fn protondb_accept_suggestion(
    request: AcceptSuggestionRequest,
    app: AppState<'_>,
) -> IpcResult<AcceptSuggestionResult> {
    commit(
        app.services()
            .profiles
            .accept_protondb_suggestion(request)
            .await,
    )
}

#[tauri::command]
pub async fn protondb_dismiss_suggestion(
    profile_name: String,
    app_id: String,
    suggestion_key: String,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .protondb
        .dismiss_suggestion(profile_name, app_id, suggestion_key)
        .await)
}
