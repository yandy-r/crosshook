//! Read-only Proton migration scan.

use std::path::PathBuf;

use crosshook_core::profile::migration::{scan_proton_migrations, MigrationScanResult};
use crosshook_core::steam::discover_steam_root_candidates;

use super::ProtonMigrationService;
use crate::presenter::display_path_tilde;
use crate::services::run_blocking;
use crate::AppError;

impl ProtonMigrationService {
    /// Scans all profiles for stale Proton paths and returns migration
    /// suggestions. Read-only: nothing is written.
    ///
    /// `steam_client_install_path` is advisory (A-5): when it is blank or has
    /// no `steamapps` subdirectory, the default client install path is used
    /// instead. Every path string in the result is display-sanitized with
    /// [`display_path_tilde`] (A-4).
    ///
    /// # Errors
    /// `Internal` when a blocking task panics or is cancelled.
    pub async fn check(
        &self,
        steam_client_install_path: Option<String>,
    ) -> Result<MigrationScanResult, AppError> {
        // A-5: validate the caller-supplied steam path; fall back to the default if invalid.
        let supplied_path = run_blocking(move || {
            Ok(steam_client_install_path.filter(|p| {
                let candidate = PathBuf::from(p.trim());
                !candidate.as_os_str().is_empty() && candidate.join("steamapps").is_dir()
            }))
        })
        .await?;
        let resolved_path = match supplied_path {
            Some(path) => path,
            None => self.steam.default_client_install_path().await,
        };

        let profiles = self.profiles.clone();
        run_blocking(move || {
            let mut diagnostics = Vec::new();
            let steam_root_candidates =
                discover_steam_root_candidates(resolved_path, &mut diagnostics);
            let mut result =
                scan_proton_migrations(&profiles, &steam_root_candidates, &mut diagnostics);

            // A-4: sanitize all IPC-bound path strings before returning.
            for suggestion in &mut result.suggestions {
                suggestion.old_path = display_path_tilde(&suggestion.old_path);
                suggestion.new_path = display_path_tilde(&suggestion.new_path);
            }
            for unmatched in &mut result.unmatched {
                unmatched.stale_path = display_path_tilde(&unmatched.stale_path);
            }
            for install_info in &mut result.installed_proton_versions {
                install_info.path = display_path_tilde(&install_info.path);
            }

            Ok(result)
        })
        .await
    }
}
