//! Provider listing and catalog reads, plus the settings flags every protonup
//! operation consults.

use crosshook_core::protonup::providers::{describe_providers, ProtonUpProviderDescriptor};
use crosshook_core::protonup::ProtonUpCatalogResponse;

use super::{ProtonUpService, DEFAULT_PROVIDER_ID};
use crate::services::run_blocking;
use crate::AppError;

/// Settings that gate protonup behaviour.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ProtonFlags {
    /// `offline_mode`: no installs, no forced catalog refresh.
    pub(super) offline_mode: bool,
    /// `protonup_include_prereleases`: whether the catalog lists prereleases.
    pub(super) include_prereleases: bool,
}

impl ProtonUpService {
    /// Reads the settings flags; an unreadable settings file yields both
    /// flags off (today's behaviour), with the failure logged.
    pub(super) async fn flags(&self) -> ProtonFlags {
        let settings = self.settings.clone();
        let loaded = run_blocking(move || settings.load().map_err(AppError::from)).await;
        match loaded {
            Ok(data) => ProtonFlags {
                offline_mode: data.offline_mode,
                include_prereleases: data.protonup_include_prereleases,
            },
            Err(error) => {
                tracing::warn!(%error, "could not read settings; using protonup defaults");
                ProtonFlags::default()
            }
        }
    }

    /// Static metadata of every registered Proton release provider.
    pub fn list_providers(&self) -> Vec<ProtonUpProviderDescriptor> {
        describe_providers()
    }

    /// Versions available from `provider` (default `ge-proton`).
    ///
    /// Reads the cache first; stale and offline markers come from the
    /// catalog source. In offline mode `force_refresh` is ignored, so the
    /// caller never forces a live fetch.
    ///
    /// # Errors
    /// Reserved for task failures; catalog problems are reported through the
    /// response's cache markers.
    pub async fn list_available_versions(
        &self,
        provider: Option<String>,
        force_refresh: bool,
    ) -> Result<ProtonUpCatalogResponse, AppError> {
        let flags = self.flags().await;
        let provider_id = provider.as_deref().unwrap_or(DEFAULT_PROVIDER_ID);
        let force_refresh = force_refresh && !flags.offline_mode;
        Ok(self
            .catalog
            .versions(provider_id, force_refresh, flags.include_prereleases)
            .await)
    }
}
