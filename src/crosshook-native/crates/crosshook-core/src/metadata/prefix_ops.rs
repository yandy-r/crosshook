use super::{prefix_deps_store, prefix_storage_store, MetadataStore, MetadataStoreError};
use crate::metadata::models::{
    PrefixDependencyStateRow, PrefixStorageCleanupAuditRow, PrefixStorageSnapshotRow,
};

impl MetadataStore {
    pub fn upsert_prefix_dep_state(
        &self,
        profile_id: &str,
        package_name: &str,
        prefix_path: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("upsert prefix dep state", |conn| {
            prefix_deps_store::upsert_dependency_state(
                conn,
                profile_id,
                package_name,
                prefix_path,
                state,
                error,
            )
        })
    }

    pub fn load_prefix_dep_states(
        &self,
        profile_id: &str,
    ) -> Result<Vec<PrefixDependencyStateRow>, MetadataStoreError> {
        self.with_conn("load prefix dep states", |conn| {
            prefix_deps_store::load_dependency_states(conn, profile_id)
        })
    }

    pub fn load_prefix_dep_state(
        &self,
        profile_id: &str,
        package_name: &str,
        prefix_path: &str,
    ) -> Result<Option<PrefixDependencyStateRow>, MetadataStoreError> {
        self.with_conn("load prefix dep state", |conn| {
            prefix_deps_store::load_dependency_state(conn, profile_id, package_name, prefix_path)
        })
    }

    pub fn clear_prefix_dep_states(&self, profile_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("clear prefix dep states", |conn| {
            prefix_deps_store::clear_dependency_states(conn, profile_id)
        })
    }

    pub fn clear_stale_prefix_dep_states(&self, ttl_hours: i64) -> Result<u64, MetadataStoreError> {
        self.with_conn_mut("clear stale prefix dep states", |conn| {
            prefix_deps_store::clear_stale_states(conn, ttl_hours)
        })
    }

    pub fn insert_prefix_storage_snapshot(
        &self,
        row: &PrefixStorageSnapshotRow,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("insert prefix storage snapshot", |conn| {
            prefix_storage_store::insert_snapshot(conn, row)
        })
    }

    pub fn list_latest_prefix_storage_snapshots(
        &self,
        limit: usize,
    ) -> Result<Vec<PrefixStorageSnapshotRow>, MetadataStoreError> {
        self.with_conn("list prefix storage snapshots", |conn| {
            prefix_storage_store::list_latest_snapshots(conn, limit)
        })
    }

    pub fn insert_prefix_storage_cleanup_audit(
        &self,
        row: &PrefixStorageCleanupAuditRow,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("insert prefix storage cleanup audit", |conn| {
            prefix_storage_store::insert_cleanup_audit(conn, row)
        })
    }

    pub fn list_prefix_storage_cleanup_audit(
        &self,
        limit: usize,
    ) -> Result<Vec<PrefixStorageCleanupAuditRow>, MetadataStoreError> {
        self.with_conn("list prefix storage cleanup audit", |conn| {
            prefix_storage_store::list_cleanup_audit(conn, limit)
        })
    }
}
