use crosshook_core::discovery::{TrainerSearchQuery, TrainerSearchResponse};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = discovery_search_trainers
            as fn(
                TrainerSearchQuery,
                State<'_, MetadataStore>,
            ) -> Result<TrainerSearchResponse, String>;
    }
}
