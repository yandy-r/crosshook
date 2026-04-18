use super::{version_store, MetadataStore, MetadataStoreError};
use crate::metadata::models::VersionSnapshotRow;

impl MetadataStore {
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_version_snapshot(
        &self,
        profile_id: &str,
        steam_app_id: &str,
        steam_build_id: Option<&str>,
        trainer_version: Option<&str>,
        trainer_file_hash: Option<&str>,
        human_game_ver: Option<&str>,
        status: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("upsert a version snapshot", |conn| {
            version_store::upsert_version_snapshot(
                conn,
                profile_id,
                steam_app_id,
                steam_build_id,
                trainer_version,
                trainer_file_hash,
                human_game_ver,
                status,
            )
        })
    }

    pub fn lookup_latest_version_snapshot(
        &self,
        profile_id: &str,
    ) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
        self.with_conn("look up the latest version snapshot", |conn| {
            version_store::lookup_latest_version_snapshot(conn, profile_id)
        })
    }

    pub fn load_version_snapshots_for_profiles(
        &self,
    ) -> Result<Vec<VersionSnapshotRow>, MetadataStoreError> {
        self.with_conn("load version snapshots for profiles", |conn| {
            version_store::load_version_snapshots_for_profiles(conn)
        })
    }

    pub fn acknowledge_version_change(&self, profile_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("acknowledge a version change", |conn| {
            version_store::acknowledge_version_change(conn, profile_id)
        })
    }
}
