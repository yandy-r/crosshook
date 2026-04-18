use super::{health_store, HealthSnapshotRow, MetadataStore, MetadataStoreError};

impl MetadataStore {
    pub fn upsert_health_snapshot(
        &self,
        profile_id: &str,
        status: &str,
        issue_count: usize,
        checked_at: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert a health snapshot", |conn| {
            health_store::upsert_health_snapshot(conn, profile_id, status, issue_count, checked_at)
        })
    }

    pub fn load_health_snapshots(&self) -> Result<Vec<HealthSnapshotRow>, MetadataStoreError> {
        self.with_conn("load health snapshots", |conn| {
            health_store::load_health_snapshots(conn)
        })
    }

    pub fn lookup_health_snapshot(
        &self,
        profile_id: &str,
    ) -> Result<Option<HealthSnapshotRow>, MetadataStoreError> {
        self.with_conn("look up a health snapshot", |conn| {
            health_store::lookup_health_snapshot(conn, profile_id)
        })
    }
}
