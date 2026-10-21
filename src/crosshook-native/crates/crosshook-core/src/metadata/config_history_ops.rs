use super::{config_history_hmac, config_history_store, MetadataStore, MetadataStoreError};
use crate::metadata::models::{ConfigRevisionRow, ConfigRevisionSource, RevisionIntegrity};

impl MetadataStore {
    /// Insert a config revision for the profile, skipping if the hash matches the
    /// latest recorded revision. Returns the new id when inserted, `None` on dedup.
    ///
    /// File-backed stores sign the revision with the config-history HMAC key
    /// (ADR-0005). Without a key the revision is still inserted, unsigned.
    pub fn insert_config_revision(
        &self,
        profile_id: &str,
        profile_name_at_write: &str,
        source: ConfigRevisionSource,
        content_hash: &str,
        snapshot_toml: &str,
        source_revision_id: Option<i64>,
        max_revisions: usize,
    ) -> Result<Option<i64>, MetadataStoreError> {
        let signer = self.config_history_key.as_deref();
        if signer.is_none() && self.is_file_backed() && self.is_available() {
            tracing::warn!(
                profile_id,
                "config history HMAC key unavailable; inserting unsigned config revision"
            );
        }
        self.with_conn_mut("insert a config revision", |conn| {
            config_history_store::insert_config_revision(
                conn,
                profile_id,
                profile_name_at_write,
                source,
                content_hash,
                snapshot_toml,
                source_revision_id,
                max_revisions,
                signer,
            )
        })
    }

    /// Verify a stored revision before it is restored (ADR-0005).
    ///
    /// Always recomputes the SHA-256 content hash. With a key loaded, every
    /// revision written after the key's signing epoch must carry a valid
    /// current-key signature: stripped, foreign, unknown-format, or
    /// mismatching signatures return [`MetadataStoreError::IntegrityCheckFailed`].
    /// Pre-epoch rows (legacy/unsigned or signed by an earlier key), derived
    /// rows carrying `hash_only` trust, and stores without a key degrade to
    /// [`RevisionIntegrity::HashOnly`] with a warning.
    pub fn verify_config_revision(
        &self,
        revision: &ConfigRevisionRow,
    ) -> Result<RevisionIntegrity, MetadataStoreError> {
        let integrity =
            config_history_hmac::verify_row(self.config_history_key.as_deref(), revision)?;
        if let RevisionIntegrity::HashOnly { reason } = integrity {
            tracing::warn!(
                revision_id = revision.id,
                profile_id = %revision.profile_id,
                reason = reason.as_str(),
                "config revision HMAC not verified; falling back to content-hash check"
            );
        }
        Ok(integrity)
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
