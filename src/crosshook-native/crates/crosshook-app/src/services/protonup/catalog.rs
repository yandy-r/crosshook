//! Provider listing and catalog reads, plus the settings flags every protonup
//! operation consults.

use crosshook_core::protonup::providers::{describe_providers, ProtonUpProviderDescriptor};
use crosshook_core::protonup::ProtonUpCatalogResponse;

use super::messages::{MSG_OFFLINE, MSG_SETTINGS_UNREADABLE};
use super::{ProtonUpService, DEFAULT_PROVIDER_ID};
use crate::services::run_blocking;
use crate::AppError;

/// Settings that gate protonup behaviour.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ProtonFlags {
    /// `offline_mode`: no installs, no forced catalog refresh.
    pub(super) offline_mode: bool,
    /// The settings file could not be read: network-starting operations fail
    /// closed because `offline_mode` is unknown.
    pub(super) settings_unreadable: bool,
    /// `protonup_include_prereleases`: whether the catalog lists prereleases.
    pub(super) include_prereleases: bool,
}

impl ProtonFlags {
    /// Fixed refusal text when network-starting work must not begin.
    pub(super) fn network_block(&self) -> Option<&'static str> {
        if self.offline_mode {
            Some(MSG_OFFLINE)
        } else if self.settings_unreadable {
            Some(MSG_SETTINGS_UNREADABLE)
        } else {
            None
        }
    }
}

impl ProtonUpService {
    /// Reads the settings flags. An unreadable or corrupt settings file sets
    /// `settings_unreadable` (callers fail closed instead of treating
    /// `offline_mode` as off) and keeps the lenient prerelease default; the
    /// failure is logged.
    pub(super) async fn flags(&self) -> ProtonFlags {
        let settings = self.settings.clone();
        let loaded = run_blocking(move || settings.load().map_err(AppError::from)).await;
        match loaded {
            Ok(data) => ProtonFlags {
                offline_mode: data.offline_mode,
                settings_unreadable: false,
                include_prereleases: data.protonup_include_prereleases,
            },
            Err(error) => {
                tracing::warn!(%error, "could not read settings; failing closed for network use");
                ProtonFlags {
                    settings_unreadable: true,
                    ..ProtonFlags::default()
                }
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
    /// catalog source. In offline mode, or when the settings cannot be read,
    /// `force_refresh` is ignored, so the caller never forces a live fetch.
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
        let force_refresh = force_refresh && !flags.offline_mode && !flags.settings_unreadable;
        Ok(self
            .catalog
            .versions(provider_id, force_refresh, flags.include_prereleases)
            .await)
    }
}
