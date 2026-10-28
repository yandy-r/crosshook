//! Install root listing and the community-version suggestion.

use std::path::PathBuf;
use std::sync::Arc;

use crosshook_core::protonup::install_root::InstallRootCandidate;
use crosshook_core::protonup::matching::match_community_version;
use crosshook_core::protonup::ProtonUpSuggestion;
use crosshook_core::steam::{discover_compat_tools, discover_steam_root_candidates};

use super::ProtonUpService;
use crate::services::run_blocking;
use crate::AppError;

impl ProtonUpService {
    /// Candidate `compatibilitytools.d` install roots, including writability.
    ///
    /// `steam_client_install_path` derives one extra candidate; `None` relies
    /// on the home-relative defaults. This is a listing only: the result
    /// never authorizes a deletion.
    ///
    /// # Errors
    /// Task failures only.
    pub async fn resolve_install_roots(
        &self,
        steam_client_install_path: Option<String>,
    ) -> Result<Vec<InstallRootCandidate>, AppError> {
        let roots = Arc::clone(&self.roots);
        run_blocking(move || {
            let steam_root = steam_client_install_path.map(PathBuf::from);
            Ok(roots.install_roots(steam_root.as_deref()))
        })
        .await
    }

    /// Compares a community profile's Proton requirement with the installed
    /// runtimes. Uses `steam_client_install_path` when given, else the
    /// default Steam client path.
    ///
    /// # Errors
    /// Task failures only.
    pub async fn get_suggestion(
        &self,
        community_version: String,
        steam_client_install_path: Option<String>,
    ) -> Result<ProtonUpSuggestion, AppError> {
        let configured_path = match steam_client_install_path {
            Some(path) => path,
            None => self.steam.default_client_install_path().await,
        };
        run_blocking(move || {
            let mut diagnostics = Vec::new();
            let steam_root_candidates =
                discover_steam_root_candidates(configured_path, &mut diagnostics);
            let installs = discover_compat_tools(&steam_root_candidates, &mut diagnostics);
            for entry in &diagnostics {
                tracing::debug!(entry, "protonup suggestion diagnostic");
            }
            Ok(match_community_version(&community_version, &installs))
        })
        .await
    }
}
