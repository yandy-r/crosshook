use super::{config_history_store, MetadataStore, MetadataStoreError};
use crate::metadata::models::{ConfigRevisionRow, ConfigRevisionSource};

impl MetadataStore {
    /// Insert a config revision for the profile, skipping if the hash matches the
    /// latest recorded revision. Returns the new id when inserted, `None` on dedup.
    pub fn insert_config_revision(
        &self,
        profile_id: &str,
        profile_name_at_write: &str,
        source: ConfigRevisionSource,
        content_hash: &str,
        snapshot_toml: &str,
        source_revision_id: Option<i64>,
    ) -> Result<Option<i64>, MetadataStoreError> {
        self.with_conn_mut("insert a config revision", |conn| {
            config_history_store::insert_config_revision(
                conn,
                profile_id,
                profile_name_at_write,
                source,
                content_hash,
                snapshot_toml,
                source_revision_id,
            )
        })
    }

    /// List config revisions for a profile ordered newest first.
    /// `limit` defaults to `MAX_CONFIG_REVISIONS_PER_PROFILE` when `None`.
    pub fn list_config_revisions(
        &self,
        profile_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ConfigRevisionRow>, MetadataStoreError> {
        self.with_conn("list config revisions", |conn| {
            config_history_store::list_config_revisions(conn, profile_id, limit)
        })
    }

    /// Get a single config revision by id, scoped to `profile_id`.
    /// Returns `None` if not found or the revision belongs to a different profile.
    pub fn get_config_revision(
        &self,
        profile_id: &str,
        revision_id: i64,
    ) -> Result<Option<ConfigRevisionRow>, MetadataStoreError> {
        self.with_conn("get a config revision", |conn| {
            config_history_store::get_config_revision(conn, profile_id, revision_id)
        })
    }

    /// Mark a revision as known-good for the profile, clearing the marker on all
    /// other revisions for that profile (single-active-marker semantics).
    pub fn set_known_good_revision(
        &self,
        profile_id: &str,
        revision_id: i64,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("set a known-good config revision", |conn| {
            config_history_store::set_known_good_revision(conn, profile_id, revision_id)
        })
    }

    /// Clear the known-good marker from all revisions for the given profile.
    pub fn clear_known_good_revision(&self, profile_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("clear known-good config revision markers", |conn| {
            config_history_store::clear_known_good_revision(conn, profile_id)
        })
    }
}
