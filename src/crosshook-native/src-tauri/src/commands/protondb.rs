use crosshook_app::services::profiles::ProfileCommit;
use crosshook_app::ProfileWriter;
use crosshook_core::launch::catalog::global_catalog;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::protondb::{
    derive_suggestions, lookup_protondb, AcceptSuggestionRequest, AcceptSuggestionResult,
    ProtonDbLookupResult, ProtonDbSuggestionSet,
};
use tauri::State;

/// Number of days a dismissed suggestion is retained before automatic eviction.
const SUGGESTION_DISMISSAL_RETENTION_DAYS: u32 = 30;

#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}

#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<ProtonDbSuggestionSet, String> {
    let metadata_store_inner = metadata_store.inner().clone();

    let lookup_result = lookup_protondb(
        &metadata_store_inner,
        &app_id,
        force_refresh.unwrap_or(false),
    )
    .await;

    let profile = profile_store
        .load(&profile_name)
        .map_err(|e| e.to_string())?;

    let catalog = global_catalog();

    let dismissed_keys = if metadata_store.is_available() {
        let profile_id = metadata_store
            .lookup_profile_id(&profile_name)
            .map_err(|e| e.to_string())?;
        if let Some(pid) = profile_id {
            metadata_store
                .get_dismissed_keys(&pid, &app_id)
                .map_err(|e| e.to_string())?
        } else {
            std::collections::HashSet::new()
        }
    } else {
        std::collections::HashSet::new()
    };

    Ok(derive_suggestions(
        &lookup_result,
        &profile,
        &catalog.entries,
        &dismissed_keys,
    ))
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
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn protondb_dismiss_suggestion(
    profile_name: String,
    app_id: String,
    suggestion_key: String,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        return Err("profile_name is required".to_string());
    }

    // Load the profile to resolve the profile_id via the metadata store.
    // We don't need the profile itself — just confirm it exists and get the id.
    let _ = profile_store
        .load(profile_name)
        .map_err(|e| e.to_string())?;

    let profile_id = metadata_store
        .lookup_profile_id(profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("profile '{profile_name}' has no metadata record — cannot dismiss suggestion")
        })?;

    metadata_store
        .dismiss_suggestion(
            &profile_id,
            &app_id,
            &suggestion_key,
            SUGGESTION_DISMISSAL_RETENTION_DAYS,
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}
