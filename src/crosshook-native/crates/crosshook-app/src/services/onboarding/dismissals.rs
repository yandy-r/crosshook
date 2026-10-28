use std::convert::Infallible;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::onboarding::global_readiness_catalog;

use super::OnboardingService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Legacy nags whose source of truth remains a settings.toml timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyNag {
    /// Missing umu-run installation guidance.
    UmuInstall,
    /// Steam Deck compatibility caveats.
    SteamDeckCaveats,
}

impl LegacyNag {
    fn id(self) -> &'static str {
        match self {
            Self::UmuInstall => "umu_run",
            Self::SteamDeckCaveats => "steam_deck_caveats",
        }
    }
}

impl OnboardingService {
    /// Marks onboarding complete in settings.toml.
    pub async fn dismiss_onboarding(&self) -> Result<(), AppError> {
        let settings = self.settings.clone();
        run_blocking(move || {
            let result = settings.update(|settings| {
                settings.onboarding_completed = true;
                Ok::<(), Infallible>(())
            })?;
            match result {
                Ok(()) => Ok(()),
                Err(never) => match never {},
            }
        })
        .await
    }

    /// Saves the legacy timestamp first, then best-effort mirrors it to SQLite.
    pub async fn dismiss_legacy_nag(&self, nag: LegacyNag) -> Result<(), AppError> {
        let service = self.clone();
        run_blocking(move || {
            let result = service.settings.update(|settings| {
                let timestamp = Some(chrono::Utc::now().to_rfc3339());
                match nag {
                    LegacyNag::UmuInstall => settings.install_nag_dismissed_at = timestamp,
                    LegacyNag::SteamDeckCaveats => settings.steam_deck_caveats_dismissed_at = timestamp,
                }
                Ok::<(), Infallible>(())
            })?;
            match result {
                Ok(()) => (),
                Err(never) => match never {},
            }
            if service.metadata.is_available() {
                if let Err(error) = service.metadata.dismiss_readiness_nag(nag.id(), 3650) {
                    tracing::warn!(%error, tool_id = nag.id(), "failed to persist SQLite readiness nag dismissal after settings save");
                }
            } else {
                tracing::warn!(tool_id = nag.id(), "readiness nag dismissal persisted to settings.toml only; SQLite metadata store unavailable");
            }
            Ok(())
        })
        .await
    }

    /// Dismisses a catalog tool nag in SQLite, with a maximum ten-year lifetime.
    pub async fn dismiss_readiness_nag(
        &self,
        tool_id: String,
        ttl_days: Option<u32>,
    ) -> Result<(), AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            require_readiness_metadata(&metadata)?;
            if global_readiness_catalog().find_by_id(&tool_id).is_none() {
                return Err(AppError::validation("tool_id", format!(
                    "unknown readiness tool_id: {tool_id}. Use a tool id from the host readiness catalog."
                )));
            }
            metadata.dismiss_readiness_nag(&tool_id, ttl_days.unwrap_or(90).min(3650))?;
            Ok(())
        })
        .await
    }
}

pub(super) fn require_readiness_metadata(metadata: &MetadataStore) -> Result<(), AppError> {
    if metadata.is_available() {
        Ok(())
    } else {
        Err(AppError::new(AppErrorKind::Unavailable,
            "readiness nag dismissals require the SQLite metadata store; dismiss_umu_install_nag and dismiss_steam_deck_caveats remain available via settings.toml"))
    }
}
