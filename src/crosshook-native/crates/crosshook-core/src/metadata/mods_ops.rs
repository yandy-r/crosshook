use chrono::Utc;

use super::{mods_store, MetadataStore, MetadataStoreError};
use crate::mods::{ProfileModInput, ProfileModRecord};

impl MetadataStore {
    pub fn list_profile_mods(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ProfileModRecord>, MetadataStoreError> {
        self.with_sqlite_conn("list profile mods", |conn| {
            mods_store::list_profile_mods(conn, profile_id)
        })
    }

    pub fn list_enabled_profile_mods(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ProfileModRecord>, MetadataStoreError> {
        self.with_sqlite_conn("list enabled profile mods", |conn| {
            mods_store::list_enabled_profile_mods(conn, profile_id)
        })
    }

    pub fn add_profile_mod(
        &self,
        profile_id: &str,
        input: &ProfileModInput,
    ) -> Result<ProfileModRecord, MetadataStoreError> {
        let normalized = mods_store::validate_mod_input(input)?;
        let now = Utc::now().to_rfc3339();
        self.with_sqlite_conn("add profile mod", |conn| {
            mods_store::insert_profile_mod(conn, profile_id, &normalized, &now)
        })
    }

    pub fn update_profile_mod(
        &self,
        profile_id: &str,
        mod_id: &str,
        input: &ProfileModInput,
    ) -> Result<ProfileModRecord, MetadataStoreError> {
        let normalized = mods_store::validate_mod_input(input)?;
        let now = Utc::now().to_rfc3339();
        self.with_sqlite_conn("update profile mod", |conn| {
            mods_store::update_profile_mod(conn, profile_id, mod_id, &normalized, &now)
        })
    }

    pub fn remove_profile_mod(
        &self,
        profile_id: &str,
        mod_id: &str,
    ) -> Result<bool, MetadataStoreError> {
        self.with_sqlite_conn("remove profile mod", |conn| {
            mods_store::delete_profile_mod(conn, profile_id, mod_id)
        })
    }
}
