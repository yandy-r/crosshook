use crosshook_core::discovery::{matching, VersionMatchResult};

use super::DiscoveryService;
use crate::services::run_blocking;
use crate::AppError;

impl DiscoveryService {
    /// Compares the trainer version with the latest installed-game snapshot.
    pub async fn check_version_compatibility(
        &self,
        profile_name: String,
        trainer_game_version: Option<String>,
    ) -> Result<VersionMatchResult, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = metadata.lookup_profile_id(&profile_name)?.ok_or_else(|| {
                AppError::validation("profile_name", format!("unknown profile: {profile_name}"))
            })?;
            let installed = metadata
                .lookup_latest_version_snapshot(&profile_id)?
                .and_then(|snapshot| snapshot.human_game_ver);
            Ok(matching::match_trainer_version(
                trainer_game_version.as_deref(),
                installed.as_deref(),
            ))
        })
        .await
    }
}
