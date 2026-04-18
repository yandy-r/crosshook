use std::path::Path;

use rusqlite::params_from_iter;

use super::util::in_clause_placeholders;
use super::{profile_sync, MetadataStore, MetadataStoreError};
use crate::metadata::models::{SyncReport, SyncSource};
use crate::profile::{GameProfile, ProfileStore};

impl MetadataStore {
    pub fn observe_profile_write(
        &self,
        name: &str,
        profile: &GameProfile,
        path: &Path,
        source: SyncSource,
        source_profile_id: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile write", |conn| {
            profile_sync::observe_profile_write(
                conn,
                name,
                profile,
                path,
                source,
                source_profile_id,
            )
        })
    }

    pub fn lookup_profile_id(&self, name: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("look up a profile id", |conn| {
            profile_sync::lookup_profile_id(conn, name)
        })
    }

    pub fn query_profile_ids_for_names(
        &self,
        profile_names: &[String],
    ) -> Result<Vec<(String, String)>, MetadataStoreError> {
        if profile_names.is_empty() {
            return Ok(Vec::new());
        }

        self.with_conn("query profile ids for names", |conn| {
            let placeholders = in_clause_placeholders(profile_names.len());
            let sql = format!(
                "SELECT current_filename, profile_id \
                 FROM profiles \
                 WHERE deleted_at IS NULL AND current_filename IN ({placeholders})"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare query_profile_ids_for_names statement",
                    source,
                })?;
            let rows = stmt
                .query_map(params_from_iter(profile_names.iter()), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|source| MetadataStoreError::Database {
                    action: "execute query_profile_ids_for_names",
                    source,
                })?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|source| MetadataStoreError::Database {
                    action: "read a query_profile_ids_for_names row",
                    source,
                })?);
            }
            Ok(result)
        })
    }

    pub fn observe_profile_rename(
        &self,
        old_name: &str,
        new_name: &str,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile rename", |conn| {
            profile_sync::observe_profile_rename(conn, old_name, new_name, old_path, new_path)
        })
    }

    pub fn observe_profile_delete(&self, name: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile delete", |conn| {
            profile_sync::observe_profile_delete(conn, name)
        })
    }

    pub fn sync_profiles_from_store(
        &self,
        store: &ProfileStore,
    ) -> Result<SyncReport, MetadataStoreError> {
        self.with_conn("sync profiles from store", |conn| {
            profile_sync::sync_profiles_from_store(conn, store)
        })
    }
}
