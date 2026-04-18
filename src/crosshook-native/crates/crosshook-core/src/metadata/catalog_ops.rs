use super::{
    optimization_catalog_store, proton_catalog_store, readiness_catalog_store,
    readiness_snapshot_store, HostReadinessSnapshotRow, MetadataStore, MetadataStoreError,
    ProtonCatalogRow,
};

impl MetadataStore {
    pub fn persist_optimization_catalog(
        &self,
        entries: &[crate::launch::catalog::OptimizationEntry],
        catalog_version: u32,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("persist optimization catalog", |conn| {
            optimization_catalog_store::persist_optimization_catalog(conn, entries, catalog_version)
        })
    }

    pub fn persist_readiness_catalog(
        &self,
        entries: &[crate::onboarding::HostToolEntry],
        catalog_version: u32,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("persist readiness catalog", |conn| {
            readiness_catalog_store::persist_readiness_catalog(conn, entries, catalog_version)
        })
    }

    pub fn upsert_host_readiness_snapshot(
        &self,
        tool_checks: &[crate::onboarding::HostToolCheckResult],
        detected_distro_family: &str,
        all_passed: bool,
        critical_failures: usize,
        warnings: usize,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("upsert host readiness snapshot", |conn| {
            readiness_snapshot_store::upsert_host_readiness_snapshot_impl(
                conn,
                tool_checks,
                detected_distro_family,
                all_passed,
                critical_failures,
                warnings,
            )
        })
    }

    pub fn get_host_readiness_snapshot(
        &self,
    ) -> Result<Option<HostReadinessSnapshotRow>, MetadataStoreError> {
        self.with_conn("get host readiness snapshot", |conn| {
            readiness_snapshot_store::get_host_readiness_snapshot_impl(conn)
        })
    }

    /// Batch-upsert rows into `proton_release_catalog`.
    pub fn put_proton_catalog(&self, rows: &[ProtonCatalogRow]) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("put proton catalog", |conn| {
            proton_catalog_store::put_proton_catalog_impl(conn, rows)
        })
    }

    /// Return all cached rows for `provider_id`, ordered by `fetched_at` descending.
    pub fn get_proton_catalog(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ProtonCatalogRow>, MetadataStoreError> {
        self.with_conn("get proton catalog", |conn| {
            proton_catalog_store::get_proton_catalog_impl(conn, provider_id)
        })
    }

    /// Delete all cached rows for `provider_id` (evict stale entries before a refresh).
    pub fn clear_proton_catalog(&self, provider_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("clear proton catalog", |conn| {
            proton_catalog_store::clear_proton_catalog_impl(conn, provider_id)
        })
    }

    /// Atomically replace all cached rows for a provider.
    pub fn replace_proton_catalog(
        &self,
        provider_id: &str,
        rows: &[ProtonCatalogRow],
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("replace proton catalog", |conn| {
            proton_catalog_store::replace_proton_catalog_impl(conn, provider_id, rows)
        })
    }
}
