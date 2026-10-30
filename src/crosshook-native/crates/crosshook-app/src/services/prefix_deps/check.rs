//! Tool detection and the read-only `list-installed` check. The check takes
//! no prefix key (D5): it never mutates the prefix.

use chrono::Utc;
use crosshook_core::prefix_deps::runner::check_installed;
use crosshook_core::prefix_deps::{BinaryDetectionResult, DependencyState, PrefixDependencyStatus};
use crosshook_core::settings::AppSettingsData;

use super::messages::{deps_error, tool_error};
use super::tools::{profile_steam_app_id, resolve_tool, ResolvedTool};
use super::PrefixDepsService;
use crate::services::run_blocking;
use crate::AppError;

impl PrefixDepsService {
    /// Detects winetricks/protontricks, honouring the settings override path.
    ///
    /// # Errors
    /// The settings store's load error.
    pub async fn detect_binary(&self) -> Result<BinaryDetectionResult, AppError> {
        let service = self.clone();
        run_blocking(move || {
            let settings = service.settings.load()?;
            Ok(service.tools.detect(&settings.protontricks_binary_path))
        })
        .await
    }

    /// Checks which of `packages` are installed in `prefix_path` and persists
    /// each package's state (fail-soft).
    ///
    /// # Errors
    /// `Unavailable` [`MSG_NO_TOOL_CHECK`](super::MSG_NO_TOOL_CHECK) without a
    /// tool, `Validation { field: "steam_app_id" }` when only protontricks is
    /// available and the profile has no valid App ID, and the check's own
    /// failure (missing prefix, tool failure, timeout).
    pub async fn check(
        &self,
        profile_name: String,
        prefix_path: String,
        packages: Vec<String>,
    ) -> Result<Vec<PrefixDependencyStatus>, AppError> {
        let tool = self.select_tool(profile_name.clone()).await?;
        let installed = check_installed(
            &tool.binary_path,
            &prefix_path,
            tool.tool,
            tool.steam_app_id.as_deref(),
        )
        .await
        .map_err(|error| deps_error("prefix_path", error))?;

        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = metadata
                .lookup_profile_id(&profile_name)
                .ok()
                .flatten()
                .unwrap_or_else(|| profile_name.clone());
            let statuses = packages
                .iter()
                .map(|package| {
                    let is_installed = installed.contains(package);
                    let state = if is_installed { "installed" } else { "missing" };
                    if let Err(error) = metadata.upsert_prefix_dep_state(
                        &profile_id,
                        package,
                        &prefix_path,
                        state,
                        None,
                    ) {
                        tracing::warn!(%error, package, "failed to persist prefix dep state");
                    }
                    let now = Utc::now().to_rfc3339();
                    PrefixDependencyStatus {
                        package_name: package.clone(),
                        state: if is_installed {
                            DependencyState::Installed
                        } else {
                            DependencyState::Missing
                        },
                        checked_at: Some(now.clone()),
                        installed_at: is_installed.then_some(now),
                        last_error: None,
                    }
                })
                .collect();
            Ok(statuses)
        })
        .await
    }

    /// Selects the check-time tool for `profile_name`.
    async fn select_tool(&self, profile_name: String) -> Result<ResolvedTool, AppError> {
        let service = self.clone();
        run_blocking(move || {
            let settings = service.settings.load()?;
            service.resolve_profile_tool(&settings, &profile_name, true)
        })
        .await
    }

    /// Selects the tool for `profile_name` (D13): detection with the settings
    /// override, the profile's Steam App ID (a profile that fails to load has
    /// none), and the winetricks fallback. `check` picks the check-time text.
    /// Blocking.
    pub(super) fn resolve_profile_tool(
        &self,
        settings: &AppSettingsData,
        profile_name: &str,
        check: bool,
    ) -> Result<ResolvedTool, AppError> {
        let detection = self.tools.detect(&settings.protontricks_binary_path);
        let steam_app_id = self
            .profiles
            .load(profile_name)
            .ok()
            .and_then(|profile| profile_steam_app_id(&profile.effective_profile()));
        resolve_tool(&detection, steam_app_id.as_deref(), || {
            self.tools.winetricks_on_path()
        })
        .map_err(|error| tool_error(error, check))
    }
}
