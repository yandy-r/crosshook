//! Proton runtime management: provider catalog, installs run as supervised
//! jobs with owner-checked cancel, and guarded uninstall.

use std::sync::Arc;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;

use crate::jobs::JobRegistry;
use crate::services::SteamService;

mod cancel;
mod catalog;
mod identity;
mod install;
mod messages;
mod owners;
mod progress;
mod roots;
mod seams;
mod types;
mod uninstall;

pub use identity::{verify_install_identity, IdentityError};
pub use progress::next_progress;
pub use seams::{ProtonCatalogSource, ProtonInstaller, ProtonRootSource};
pub use types::{
    InstallOwner, ProtonInstallHandle, ProtonInstallStart, ProtonUninstallPlanResponse,
    ProtonUninstallResponse,
};

use owners::OwnerLedger;
use seams::{CoreCatalogSource, CoreInstaller, CoreProtonRoots};

/// Registry kind for Proton install jobs.
pub const PROTONUP_INSTALL_JOB_KIND: &str = "protonup-install";
/// Provider id used when the caller omits one.
pub const DEFAULT_PROVIDER_ID: &str = "ge-proton";

/// Toolkit-independent Proton catalog, install and uninstall operations.
#[derive(Clone)]
pub struct ProtonUpService {
    settings: SettingsStore,
    steam: SteamService,
    jobs: JobRegistry,
    catalog: Arc<dyn ProtonCatalogSource>,
    installer: Arc<dyn ProtonInstaller>,
    roots: Arc<dyn ProtonRootSource>,
    owners: OwnerLedger,
}

impl ProtonUpService {
    /// Builds the service over the shared stores and job registry with the
    /// core catalog, installer and root discovery.
    pub fn new(
        metadata: MetadataStore,
        settings: SettingsStore,
        steam: SteamService,
        jobs: JobRegistry,
    ) -> Self {
        let catalog = Arc::new(CoreCatalogSource { metadata });
        let roots = Arc::new(CoreProtonRoots {
            steam: steam.clone(),
        });
        Self {
            settings,
            steam,
            jobs,
            catalog,
            installer: Arc::new(CoreInstaller),
            roots,
            owners: OwnerLedger::default(),
        }
    }

    /// Builds the service with injected seams.
    #[cfg(test)]
    pub(crate) fn with_seams(
        settings: SettingsStore,
        steam: SteamService,
        jobs: JobRegistry,
        catalog: Arc<dyn ProtonCatalogSource>,
        installer: Arc<dyn ProtonInstaller>,
        roots: Arc<dyn ProtonRootSource>,
    ) -> Self {
        Self {
            settings,
            steam,
            jobs,
            catalog,
            installer,
            roots,
            owners: OwnerLedger::default(),
        }
    }

    /// Number of installs currently registered in the owner ledger.
    #[cfg(test)]
    pub(crate) fn owner_count(&self) -> usize {
        self.owners.len()
    }

    /// The job registry this service starts installs on.
    #[cfg(test)]
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }
}

static_assertions::assert_impl_all!(ProtonUpService: Send, Sync);

#[cfg(test)]
mod tests;
