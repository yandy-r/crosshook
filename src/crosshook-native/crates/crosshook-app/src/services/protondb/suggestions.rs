use std::collections::HashSet;

use crosshook_core::launch::catalog::global_catalog;
use crosshook_core::protondb::{derive_suggestions, lookup_protondb, ProtonDbSuggestionSet};

use super::{ProtonDbService, SUGGESTION_DISMISSAL_RETENTION_DAYS};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl ProtonDbService {
    /// Derives suggestions from lookup data, the profile and retained dismissals.
    pub async fn suggestions(
        &self,
        app_id: String,
        profile_name: String,
        force_refresh: bool,
    ) -> Result<ProtonDbSuggestionSet, AppError> {
        let service = self.clone();
        let runtime = tokio::runtime::Handle::current();
        run_blocking(move || {
            let lookup =
                runtime.block_on(lookup_protondb(&service.metadata, &app_id, force_refresh));
            let profile = service.profiles.load(&profile_name)?;
            let catalog = global_catalog();
            let dismissed = if service.metadata.is_available() {
                if let Some(id) = service.metadata.lookup_profile_id(&profile_name)? {
                    service.metadata.get_dismissed_keys(&id, &app_id)?
                } else {
                    HashSet::new()
                }
            } else {
                HashSet::new()
            };
            Ok(derive_suggestions(
                &lookup,
                &profile,
                &catalog.entries,
                &dismissed,
            ))
        })
        .await
    }

    /// Retains a dismissal for thirty days without evicting other records.
    pub async fn dismiss_suggestion(
        &self,
        profile_name: String,
        app_id: String,
        suggestion_key: String,
    ) -> Result<(), AppError> {
        let service = self.clone();
        run_blocking(move || {
            let profile_name = profile_name.trim();
            if profile_name.is_empty() {
                return Err(AppError::validation(
                    "profile_name",
                    "profile_name is required",
                ));
            }
            service.profiles.load(profile_name)?;
            let id = service
                .metadata
                .lookup_profile_id(profile_name)?
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::NotFound,
                        format!(
                    "profile '{profile_name}' has no metadata record — cannot dismiss suggestion"
                ),
                    )
                })?;
            service.metadata.dismiss_suggestion(
                &id,
                &app_id,
                &suggestion_key,
                SUGGESTION_DISMISSAL_RETENTION_DAYS,
            )?;
            Ok(())
        })
        .await
    }
}
