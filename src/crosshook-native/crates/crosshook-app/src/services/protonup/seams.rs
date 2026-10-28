//! App-private seams over the core catalog, installer and root discovery, so
//! tests can run the service without network, real downloads or `$HOME`.

use std::path::{Path, PathBuf};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::protonup::catalog::list_available_versions_by_id;
use crosshook_core::protonup::install::{install_version_with_progress, InstallError};
use crosshook_core::protonup::install_root::{
    resolve_install_root_candidates, InstallRootCandidate,
};
use crosshook_core::protonup::progress::ProgressEmitter;
use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpCatalogResponse, ProtonUpInstallRequest,
    ProtonUpInstallResult,
};
use crosshook_core::steam::discover_steam_root_candidates;
use tokio_util::sync::CancellationToken;

use crate::services::{run_blocking, BoxFuture, SteamService};

/// Source of the provider version catalog.
pub trait ProtonCatalogSource: Send + Sync {
    /// Versions of `provider_id`; the default reads core's cache-first catalog.
    fn versions<'a>(
        &'a self,
        provider_id: &'a str,
        force_refresh: bool,
        include_prereleases: bool,
    ) -> BoxFuture<'a, ProtonUpCatalogResponse>;
}

/// Executor of one Proton install.
pub trait ProtonInstaller: Send + Sync {
    /// Installs `version` for `request`; the default is core's orchestrator.
    fn install<'a>(
        &'a self,
        request: &'a ProtonUpInstallRequest,
        version: &'a ProtonUpAvailableVersion,
        emitter: Option<ProgressEmitter>,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProtonUpInstallResult, InstallError>>;
}

/// Backend-trusted Steam roots and the Proton install roots derived from them.
pub trait ProtonRootSource: Send + Sync {
    /// Steam roots the backend discovered itself (never caller supplied).
    fn steam_roots(&self) -> BoxFuture<'_, Vec<PathBuf>>;

    /// Install root candidates for `steam_root`. Blocking filesystem work.
    fn install_roots(&self, steam_root: Option<&Path>) -> Vec<InstallRootCandidate>;
}

/// Default catalog: core's `list_available_versions_by_id` over the metadata store.
pub(super) struct CoreCatalogSource {
    pub(super) metadata: MetadataStore,
}

impl ProtonCatalogSource for CoreCatalogSource {
    fn versions<'a>(
        &'a self,
        provider_id: &'a str,
        force_refresh: bool,
        include_prereleases: bool,
    ) -> BoxFuture<'a, ProtonUpCatalogResponse> {
        Box::pin(list_available_versions_by_id(
            &self.metadata,
            force_refresh,
            provider_id,
            include_prereleases,
        ))
    }
}

/// Default installer: core's `install_version_with_progress`.
pub(super) struct CoreInstaller;

impl ProtonInstaller for CoreInstaller {
    fn install<'a>(
        &'a self,
        request: &'a ProtonUpInstallRequest,
        version: &'a ProtonUpAvailableVersion,
        emitter: Option<ProgressEmitter>,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProtonUpInstallResult, InstallError>> {
        Box::pin(install_version_with_progress(
            request, version, emitter, cancel,
        ))
    }
}

/// Default roots: the default Steam client path expanded by core's discovery,
/// and core's install root resolver.
pub(super) struct CoreProtonRoots {
    pub(super) steam: SteamService,
}

impl ProtonRootSource for CoreProtonRoots {
    fn steam_roots(&self) -> BoxFuture<'_, Vec<PathBuf>> {
        Box::pin(async move {
            let client_path = self.steam.default_client_install_path().await;
            let discovered = run_blocking(move || {
                let mut diagnostics = Vec::new();
                Ok(discover_steam_root_candidates(
                    client_path,
                    &mut diagnostics,
                ))
            })
            .await;
            discovered.unwrap_or_else(|error| {
                tracing::warn!(%error, "steam root discovery failed; no trusted roots");
                Vec::new()
            })
        })
    }

    fn install_roots(&self, steam_root: Option<&Path>) -> Vec<InstallRootCandidate> {
        resolve_install_root_candidates(steam_root)
    }
}
