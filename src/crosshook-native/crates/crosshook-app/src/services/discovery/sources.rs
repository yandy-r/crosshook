use crosshook_core::discovery::{
    models::validate_external_source, ExternalTrainerSourceSubscription,
};

use super::DiscoveryService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl DiscoveryService {
    /// Lists subscriptions without exposing unknown local-persistence fields.
    pub async fn list_external_sources(
        &self,
    ) -> Result<Vec<ExternalTrainerSourceSubscription>, AppError> {
        let settings = self.settings.clone();
        run_blocking(move || {
            let mut settings = settings.load()?;
            settings.clear_extra();
            Ok(settings.external_trainer_sources)
        })
        .await
    }

    /// Validates and atomically appends a source, rejecting duplicate identifiers.
    pub async fn add_external_source(
        &self,
        mut source: ExternalTrainerSourceSubscription,
    ) -> Result<Vec<ExternalTrainerSourceSubscription>, AppError> {
        let settings = self.settings.clone();
        run_blocking(move || {
            validate_external_source(&source)
                .map_err(|message| AppError::validation("external_source", message))?;
            source.extra.clear();
            settings.update(|settings| {
                if settings
                    .external_trainer_sources
                    .iter()
                    .any(|s| s.source_id == source.source_id)
                {
                    return Err(AppError::new(
                        AppErrorKind::Conflict,
                        format!("source with id {:?} already exists", source.source_id),
                    ));
                }
                settings.external_trainer_sources.push(source);
                Ok(without_extras(settings.external_trainer_sources.clone()))
            })?
        })
        .await
    }

    /// Atomically removes a source by its identifier.
    pub async fn remove_external_source(
        &self,
        source_id: String,
    ) -> Result<Vec<ExternalTrainerSourceSubscription>, AppError> {
        let settings = self.settings.clone();
        run_blocking(move || {
            settings.update(|settings| {
                let before = settings.external_trainer_sources.len();
                settings
                    .external_trainer_sources
                    .retain(|source| source.source_id != source_id);
                if settings.external_trainer_sources.len() == before {
                    return Err(AppError::new(
                        AppErrorKind::NotFound,
                        format!("no source with id {source_id:?} found"),
                    ));
                }
                Ok(without_extras(settings.external_trainer_sources.clone()))
            })?
        })
        .await
    }
}

fn without_extras(
    mut sources: Vec<ExternalTrainerSourceSubscription>,
) -> Vec<ExternalTrainerSourceSubscription> {
    for source in &mut sources {
        source.extra.clear();
    }
    sources
}
