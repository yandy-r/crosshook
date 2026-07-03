use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use super::{community_index, MetadataStore, MetadataStoreError};
use crate::community::taps::CommunityTapSyncResult;
use crate::discovery::{CatalogPage, CatalogQuery};
use crate::metadata::models::CommunityProfileRow;

impl MetadataStore {
    pub fn index_community_tap_result(
        &self,
        result: &CommunityTapSyncResult,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("index a community tap", |conn| {
            community_index::index_community_tap_result_with_trainers(conn, result)
        })
    }

    pub fn lookup_community_tap_id(
        &self,
        tap_url: &str,
        tap_branch: &str,
    ) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("look up community tap id", |conn| {
            let mut stmt = conn
                .prepare("SELECT tap_id FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2")
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare community tap id lookup",
                    source,
                })?;
            stmt.query_row(params![tap_url, tap_branch], |row| row.get(0))
                .optional()
                .map_err(|source| MetadataStoreError::Database {
                    action: "look up community tap id",
                    source,
                })
        })
    }

    /// Persists tap offline metadata after a successful sync (including cache-only sync).
    /// No-op when the tap has not been indexed yet (no `community_taps` row).
    pub fn upsert_community_tap_offline_from_sync_result(
        &self,
        result: &CommunityTapSyncResult,
    ) -> Result<(), MetadataStoreError> {
        let tap_url = &result.workspace.subscription.url;
        let tap_branch = result
            .workspace
            .subscription
            .branch
            .as_deref()
            .unwrap_or("");
        let Some(tap_id) = self.lookup_community_tap_id(tap_url, tap_branch)? else {
            return Ok(());
        };
        let now = Utc::now().to_rfc3339();
        let size = crate::community::taps::directory_size_bytes(&result.workspace.local_path);
        let last_sync_at = if result.from_cache {
            self.lookup_community_tap_offline_state_row(&tap_id)?
                .and_then(|r| r.last_sync_at)
        } else {
            Some(now)
        };
        self.upsert_community_tap_offline_state_row(
            &tap_id,
            1,
            last_sync_at.as_deref(),
            Some(size as i64),
        )
    }

    pub fn list_community_tap_profiles(
        &self,
        tap_url: Option<&str>,
    ) -> Result<Vec<CommunityProfileRow>, MetadataStoreError> {
        self.with_conn("list community tap profiles", |conn| {
            community_index::list_community_tap_profiles(conn, tap_url)
        })
    }

    /// Errors when the store is unavailable (no silent empty page) so the
    /// command layer can fall back to the degraded filesystem catalog.
    pub fn query_community_catalog(
        &self,
        query: &CatalogQuery,
    ) -> Result<CatalogPage, MetadataStoreError> {
        self.with_sqlite_conn("query the community catalog", |conn| {
            community_index::query_community_catalog(conn, query)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_community_catalog_errors_when_store_unavailable() {
        let store = MetadataStore::disabled();
        let result = store.query_community_catalog(&CatalogQuery::default());
        assert!(
            result.is_err(),
            "unavailable store must error so the degraded fallback runs instead of \
             returning an empty non-degraded page"
        );
    }
}
