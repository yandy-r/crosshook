use std::path::Path;

use super::{
    offline_store, CommunityTapOfflineRow, MetadataStore, MetadataStoreError, OfflineReadinessRow,
};
use crate::profile::GameProfile;

impl MetadataStore {
    pub fn check_offline_readiness_for_profile(
        &self,
        profile_name: &str,
        profile_id: &str,
        profile: &GameProfile,
    ) -> Result<crate::offline::OfflineReadinessReport, MetadataStoreError> {
        self.with_sqlite_conn("check offline readiness", |conn| {
            crate::offline::check_offline_preflight(profile_name, profile_id, profile, conn)
        })
    }

    pub fn verify_trainer_hash_for_profile_path(
        &self,
        profile_id: &str,
        trainer_path: &Path,
    ) -> Result<Option<crate::offline::HashVerifyResult>, MetadataStoreError> {
        self.with_conn("verify trainer hash", |conn| {
            crate::offline::verify_and_cache_trainer_hash(conn, profile_id, trainer_path)
        })
    }

    pub fn load_offline_readiness_snapshot_rows(
        &self,
    ) -> Result<Vec<OfflineReadinessRow>, MetadataStoreError> {
        self.with_conn("load offline readiness snapshots", |conn| {
            offline_store::load_offline_readiness_snapshots(conn)
        })
    }

    pub fn upsert_community_tap_offline_state_row(
        &self,
        tap_id: &str,
        has_local_clone: i64,
        last_sync_at: Option<&str>,
        clone_size_bytes: Option<i64>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert community tap offline state", |conn| {
            offline_store::upsert_community_tap_offline_state(
                conn,
                tap_id,
                has_local_clone,
                last_sync_at,
                clone_size_bytes,
            )
        })
    }

    pub fn lookup_community_tap_offline_state_row(
        &self,
        tap_id: &str,
    ) -> Result<Option<CommunityTapOfflineRow>, MetadataStoreError> {
        self.with_conn("lookup community tap offline state", |conn| {
            offline_store::lookup_community_tap_offline_state(conn, tap_id)
        })
    }
}
