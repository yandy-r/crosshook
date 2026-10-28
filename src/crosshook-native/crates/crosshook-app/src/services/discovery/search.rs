use crosshook_core::discovery::{
    search_external_trainers_with_mode, ExternalSearchMode, ExternalTrainerSearchQuery,
    ExternalTrainerSearchResponse,
};

use super::DiscoveryService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl DiscoveryService {
    /// Searches enabled sources, using fresh or stale cache only in offline mode.
    /// Only settings loading occupies a blocking worker; core source work is awaited here.
    pub async fn search_external(
        &self,
        query: ExternalTrainerSearchQuery,
    ) -> Result<ExternalTrainerSearchResponse, AppError> {
        let settings = self.settings.clone();
        let settings = run_blocking(move || settings.load().map_err(AppError::from)).await?;
        let active = !query.game_name.trim().is_empty()
            && settings
                .external_trainer_sources
                .iter()
                .any(|source| source.enabled);
        let mode = if settings.offline_mode {
            ExternalSearchMode::CacheOnly
        } else {
            ExternalSearchMode::Online
        };
        let response = search_external_trainers_with_mode(
            &self.metadata,
            &settings.external_trainer_sources,
            &query,
            mode,
        )
        .await;
        if settings.offline_mode && active && !response.cached {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "offline mode is enabled",
            ));
        }
        Ok(response)
    }
}
