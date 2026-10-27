//! Prefix storage service: scan job, hardened cleanup and history over the profile
//! prefix inventory and the metadata audit tables.

mod audit;
mod cleanup;
mod scan;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crosshook_core::metadata::{
    MetadataStore, PrefixStorageCleanupAuditRow, PrefixStorageSnapshotRow,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::storage::PrefixStorageScanResult;
use serde::Serialize;

use crate::jobs::{JobId, JobRegistry};

/// `JobSpec::kind` of the prefix storage scan job.
pub const STORAGE_SCAN_JOB_KIND: &str = "prefix-storage-scan";
/// Exclusive `JobKey` of the prefix storage scan job.
pub const STORAGE_SCAN_JOB_KEY: &str = "prefix-storage-scan";

/// Async application service over prefix storage scan, cleanup and history.
///
/// Clones share the stores, the job registry and the scan result slot.
#[derive(Clone)]
pub struct StorageService {
    profiles: ProfileStore,
    metadata: MetadataStore,
    jobs: JobRegistry,
    scan_results: Arc<Mutex<HashMap<JobId, PrefixStorageScanResult>>>,
}

impl StorageService {
    /// Builds the service from shared stores and the app job registry.
    pub fn new(profiles: ProfileStore, metadata: MetadataStore, jobs: JobRegistry) -> Self {
        Self {
            profiles,
            metadata,
            jobs,
            scan_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    /// The job registry the scan job runs on.
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }
}

/// Prefix storage history: recent scan snapshots and cleanup audit rows.
///
/// `available` is `false` (with empty lists) when the metadata database is
/// unavailable.
#[derive(Debug, Clone, Serialize)]
pub struct PrefixStorageHistoryResponse {
    /// Whether the metadata database could be read.
    pub available: bool,
    /// Most recent scan snapshots.
    pub snapshots: Vec<PrefixStorageSnapshotRow>,
    /// Most recent cleanup audit rows.
    pub audit: Vec<PrefixStorageCleanupAuditRow>,
}

static_assertions::assert_impl_all!(StorageService: Send, Sync);

#[cfg(test)]
mod tests;
