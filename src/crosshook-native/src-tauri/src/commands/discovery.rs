use crosshook_core::discovery::{
    ExternalTrainerSearchQuery, ExternalTrainerSearchResponse, TrainerSearchQuery,
    TrainerSearchResponse, VersionMatchResult,
};
use crosshook_core::discovery::matching;
use crosshook_core::metadata::MetadataStore;
use tauri::State;

#[tauri::command]
pub fn discovery_search_trainers(
    query: TrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerSearchResponse, String> {
    metadata_store
        .search_trainer_sources(
            &query.query,
            query.limit.unwrap_or(20) as i64,
            query.offset.unwrap_or(0) as i64,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn discovery_search_external(
    query: ExternalTrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalTrainerSearchResponse, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(crosshook_core::discovery::search_external_trainers(&metadata_store, &query).await)
}

#[tauri::command]
pub fn discovery_check_version_compatibility(
    profile_name: String,
    trainer_game_version: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionMatchResult, String> {
    let Some(profile_id) = metadata_store
        .lookup_profile_id(&profile_name)
        .map_err(|e| e.to_string())?
    else {
        return Err(format!("unknown profile: {profile_name}"));
    };

    let snapshot = metadata_store
        .lookup_latest_version_snapshot(&profile_id)
        .map_err(|e| e.to_string())?;

    let installed_ver = snapshot.and_then(|s| s.human_game_ver);

    Ok(matching::match_trainer_version(
        trainer_game_version.as_deref(),
        installed_ver.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Phase A: sync search
        let _ = discovery_search_trainers
            as fn(
                TrainerSearchQuery,
                State<'_, MetadataStore>,
            ) -> Result<TrainerSearchResponse, String>;

        // Phase B: sync version compatibility check
        let _ = discovery_check_version_compatibility
            as fn(
                String,
                Option<String>,
                State<'_, MetadataStore>,
            ) -> Result<VersionMatchResult, String>;

        // Phase B: async external search — verify it exists and compiles.
        // Async fn can't be cast to a regular fn pointer, so we reference it
        // to ensure the symbol exists and the signature compiles.
        let _async_exists = discovery_search_external;
    }
}
