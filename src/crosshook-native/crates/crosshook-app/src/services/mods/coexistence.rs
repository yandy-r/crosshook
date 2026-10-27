//! Trainer/mod coexistence analysis over the registered mods.

use crosshook_core::launch::{analyze_profile_mod_coexistence, LaunchValidationIssue};

use super::crud::{resolve_profile, store_error, unavailable};
use super::ModsService;
use crate::services::run_blocking;
use crate::AppError;

impl ModsService {
    /// Analyzes the enabled registered mods of `profile_name` against its
    /// trainer setup. Store errors propagate so callers can tell "database
    /// down" from "no issues".
    pub async fn analyze(
        &self,
        profile_name: String,
    ) -> Result<Vec<LaunchValidationIssue>, AppError> {
        if !self.metadata.is_available() {
            return Err(unavailable());
        }
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let (profile, profile_id) = resolve_profile(&profiles, &metadata, &profile_name)?;
            analyze_profile_mod_coexistence(&metadata, &profile_id, &profile).map_err(store_error)
        })
        .await
    }
}
