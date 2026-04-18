use std::path::Path;

use super::{
    offline_store, CommunityTapOfflineRow, MetadataStore, MetadataStoreError, OfflineReadinessRow,
    TrainerHashCacheRow,
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

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_trainer_hash_cache_row(
        &self,
        cache_id: &str,
        profile_id: &str,
        file_path: &str,
        file_size: Option<i64>,
        file_modified_at: Option<&str>,
        sha256_hash: &str,
        verified_at: &str,
        created_at: &str,
        updated_at: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert trainer hash cache", |conn| {
            offline_store::upsert_trainer_hash_cache(
                conn,
                cache_id,
                profile_id,
                file_path,
                file_size,
                file_modified_at,
                sha256_hash,
                verified_at,
                created_at,
                updated_at,
            )
        })
    }

    pub fn lookup_trainer_hash_cache_row(
        &self,
        profile_id: &str,
        file_path: &str,
    ) -> Result<Option<TrainerHashCacheRow>, MetadataStoreError> {
        self.with_conn("lookup trainer hash cache", |conn| {
            offline_store::lookup_trainer_hash_cache(conn, profile_id, file_path)
        })
    }

    pub fn verify_trainer_hash_for_profile_path(
        &self,
        profile_id: &str,
        trainer_path: &Path,
    ) -> Result<Option<crate::offline::HashVerifyResult>, MetadataStoreError> {
        if !self.available {
            return Ok(None);
        }
        let Some(conn) = &self.conn else {
            return Ok(None);
        };
        let guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(
                "metadata store mutex poisoned while verify trainer hash".to_string(),
            )
        })?;
        crate::offline::verify_and_cache_trainer_hash(&guard, profile_id, trainer_path)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_offline_readiness_snapshot_row(
        &self,
        profile_id: &str,
        readiness_state: &str,
        readiness_score: i64,
        trainer_type: &str,
        trainer_present: i64,
        trainer_hash_valid: i64,
        trainer_activated: i64,
        proton_available: i64,
        community_tap_cached: i64,
        network_required: i64,
        blocking_reasons: Option<&str>,
        checked_at: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert offline readiness snapshot", |conn| {
            offline_store::upsert_offline_readiness_snapshot(
                conn,
                profile_id,
                readiness_state,
                readiness_score,
                trainer_type,
                trainer_present,
                trainer_hash_valid,
                trainer_activated,
                proton_available,
                community_tap_cached,
                network_required,
                blocking_reasons,
                checked_at,
            )
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
