use crosshook_core::discovery::{
    ExternalTrainerSearchQuery, ExternalTrainerSearchResponse, ExternalTrainerSourceSubscription,
    TrainerSearchQuery, TrainerSearchResponse, VersionMatchResult,
};
use crosshook_core::discovery::matching;
use crosshook_core::discovery::models::validate_external_source;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;
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
    settings_store: State<'_, SettingsStore>,
) -> Result<ExternalTrainerSearchResponse, String> {
    let settings = settings_store.load().map_err(|e| e.to_string())?;
    let metadata_store = metadata_store.inner().clone();
    Ok(
        crosshook_core::discovery::search_external_trainers(
            &metadata_store,
            &settings.external_trainer_sources,
            &query,
        )
        .await,
    )
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

#[tauri::command]
pub fn discovery_list_external_sources(
    settings_store: State<'_, SettingsStore>,
) -> Result<Vec<ExternalTrainerSourceSubscription>, String> {
    let settings = settings_store.load().map_err(|e| e.to_string())?;
    Ok(settings.external_trainer_sources)
}

#[tauri::command]
pub fn discovery_add_external_source(
    source: ExternalTrainerSourceSubscription,
    settings_store: State<'_, SettingsStore>,
) -> Result<Vec<ExternalTrainerSourceSubscription>, String> {
    validate_external_source(&source)?;

    let mut settings = settings_store.load().map_err(|e| e.to_string())?;

    if settings
        .external_trainer_sources
        .iter()
        .any(|s| s.source_id == source.source_id)
    {
        return Err(format!(
            "source with id {:?} already exists",
            source.source_id
        ));
    }

    settings.external_trainer_sources.push(source);
    settings_store
        .save(&settings)
        .map_err(|e| e.to_string())?;
    Ok(settings.external_trainer_sources)
}

#[tauri::command]
pub fn discovery_remove_external_source(
    source_id: String,
    settings_store: State<'_, SettingsStore>,
) -> Result<Vec<ExternalTrainerSourceSubscription>, String> {
    let mut settings = settings_store.load().map_err(|e| e.to_string())?;

    let before = settings.external_trainer_sources.len();
    settings
        .external_trainer_sources
        .retain(|s| s.source_id != source_id);

    if settings.external_trainer_sources.len() == before {
        return Err(format!("no source with id {:?} found", source_id));
    }

    settings_store
        .save(&settings)
        .map_err(|e| e.to_string())?;
    Ok(settings.external_trainer_sources)
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

        // Phase B: async external search (can't cast async fn to fn pointer)
        let _async_exists = discovery_search_external;

        // Source management
        let _ = discovery_list_external_sources
            as fn(
                State<'_, SettingsStore>,
            ) -> Result<Vec<ExternalTrainerSourceSubscription>, String>;

        let _ = discovery_add_external_source
            as fn(
                ExternalTrainerSourceSubscription,
                State<'_, SettingsStore>,
            ) -> Result<Vec<ExternalTrainerSourceSubscription>, String>;

        let _ = discovery_remove_external_source
            as fn(
                String,
                State<'_, SettingsStore>,
            ) -> Result<Vec<ExternalTrainerSourceSubscription>, String>;
    }
}
