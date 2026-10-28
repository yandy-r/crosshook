//! Offline readiness, trainer hash verification, network status and trainer
//! type catalog.

use std::path::PathBuf;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::offline::{
    global_trainer_type_catalog, is_network_available, HashVerifyResult, OfflineReadinessReport,
    TrainerTypeEntry,
};
use crosshook_core::profile::ProfileStore;

use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

#[cfg(test)]
mod tests;

/// Async offline-readiness service. Clones are cheap.
#[derive(Clone)]
pub struct OfflineService {
    profiles: ProfileStore,
    metadata: MetadataStore,
}

impl OfflineService {
    /// Builds the service from shared persistent stores.
    pub fn new(profiles: ProfileStore, metadata: MetadataStore) -> Self {
        Self { profiles, metadata }
    }

    /// Fails with `Unavailable` when the metadata store is disabled.
    fn ensure_metadata(&self) -> Result<(), AppError> {
        if self.metadata.is_available() {
            Ok(())
        } else {
            Err(AppError::new(
                AppErrorKind::Unavailable,
                "metadata store unavailable",
            ))
        }
    }

    /// Computes the offline readiness report for one profile.
    pub async fn check_readiness(&self, name: String) -> Result<OfflineReadinessReport, AppError> {
        self.ensure_metadata()?;
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile = profiles.load(&name)?;
            let profile_id = metadata
                .lookup_profile_id(&name)?
                .ok_or_else(|| profile_id_missing(&name))?;
            Ok(metadata.check_offline_readiness_for_profile(&name, &profile_id, &profile)?)
        })
        .await
    }

    /// Computes readiness for every saved profile, skipping (and logging)
    /// profiles that fail to load, resolve or check.
    pub async fn batch_readiness(&self) -> Result<Vec<OfflineReadinessReport>, AppError> {
        self.ensure_metadata()?;
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let names = profiles.list()?;
            let mut out = Vec::with_capacity(names.len());
            for name in names {
                let profile = match profiles.load(&name) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(profile = %name, error = %e, "skip profile in batch_offline_readiness");
                        continue;
                    }
                };
                let pid = match metadata.lookup_profile_id(&name) {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        tracing::warn!(profile = %name, "skip profile without profile_id");
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(profile = %name, error = %e, "skip profile in batch_offline_readiness (lookup failed)");
                        continue;
                    }
                };
                match metadata.check_offline_readiness_for_profile(&name, &pid, &profile) {
                    Ok(report) => out.push(report),
                    Err(e) => {
                        tracing::warn!(profile = %name, error = %e, "skip profile in batch_offline_readiness (readiness check failed)");
                    }
                }
            }
            Ok(out)
        })
        .await
    }

    /// Verifies the profile's trainer file hash against the stored baseline.
    pub async fn verify_trainer_hash(&self, name: String) -> Result<HashVerifyResult, AppError> {
        self.ensure_metadata()?;
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile = profiles.load(&name)?;
            let profile_id = metadata
                .lookup_profile_id(&name)?
                .ok_or_else(|| profile_id_missing(&name))?;
            let path = PathBuf::from(profile.effective_profile().trainer.path.trim());
            if path.as_os_str().is_empty() {
                return Err(AppError::new(
                    AppErrorKind::Validation {
                        field: "trainer_path",
                    },
                    "trainer path is empty",
                ));
            }
            metadata
                .verify_trainer_hash_for_profile_path(&profile_id, &path)?
                .ok_or_else(|| {
                    AppError::new(AppErrorKind::NotFound, "trainer file missing or unreadable")
                })
        })
        .await
    }

    /// Probes whether the network is reachable.
    pub async fn network_status(&self) -> Result<bool, AppError> {
        run_blocking(|| Ok(is_network_available())).await
    }

    /// Returns the global trainer type catalog entries.
    pub fn trainer_type_catalog(&self) -> Vec<TrainerTypeEntry> {
        global_trainer_type_catalog().entries().to_vec()
    }
}

fn profile_id_missing(name: &str) -> AppError {
    AppError::new(
        AppErrorKind::NotFound,
        format!("profile id not found for {name}"),
    )
}
