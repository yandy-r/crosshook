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

    /// Drops the cached health snapshot for `profile_id` so the next health read
    /// recomputes it. No schema change; a missing row or a disabled store is a no-op.
    /// Callers treat failures as advisory (fail-soft).
    pub fn invalidate_health_snapshot(&self, profile_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("invalidate a health snapshot", |conn| {
            health_store::delete_health_snapshot(conn, profile_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::MetadataStore;
    use crate::metadata::SyncSource;
    use crate::profile::GameProfile;
    use std::path::Path;

    fn store_with_snapshot() -> (MetadataStore, String) {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .observe_profile_write(
                "Alpha",
                &GameProfile::default(),
                Path::new("/profiles/Alpha.toml"),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
        let profile_id = store.lookup_profile_id("Alpha").unwrap().unwrap();
        store
            .upsert_health_snapshot(&profile_id, "healthy", 0, "2026-01-01T00:00:00Z")
            .unwrap();
        (store, profile_id)
    }

    #[test]
    fn invalidate_health_snapshot_removes_present_row() {
        let (store, profile_id) = store_with_snapshot();
        assert!(store.lookup_health_snapshot(&profile_id).unwrap().is_some());

        store.invalidate_health_snapshot(&profile_id).unwrap();

        assert!(store.lookup_health_snapshot(&profile_id).unwrap().is_none());
    }

    #[test]
    fn invalidate_health_snapshot_missing_row_is_noop() {
        let (store, profile_id) = store_with_snapshot();

        store.invalidate_health_snapshot("no-such-profile").unwrap();

        assert!(store.lookup_health_snapshot(&profile_id).unwrap().is_some());
    }

    #[test]
    fn invalidate_health_snapshot_disabled_store_is_noop() {
        MetadataStore::disabled()
            .invalidate_health_snapshot("any")
            .unwrap();
    }
}
