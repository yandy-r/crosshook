use super::{collections, MetadataStore, MetadataStoreError};
use crate::metadata::models::CollectionRow;
use crate::profile::CollectionDefaultsSection;

impl MetadataStore {
    pub fn list_collections(&self) -> Result<Vec<CollectionRow>, MetadataStoreError> {
        self.with_conn("list collections", |conn| {
            collections::list_collections(conn)
        })
    }

    pub fn create_collection(&self, name: &str) -> Result<String, MetadataStoreError> {
        self.with_conn("create a collection", |conn| {
            collections::create_collection(conn, name)
        })
    }

    pub fn delete_collection(&self, collection_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("delete a collection", |conn| {
            collections::delete_collection(conn, collection_id)
        })
    }

    pub fn add_profile_to_collection(
        &self,
        collection_id: &str,
        profile_name: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("add a profile to a collection", |conn| {
            collections::add_profile_to_collection(conn, collection_id, profile_name)
        })
    }

    pub fn remove_profile_from_collection(
        &self,
        collection_id: &str,
        profile_name: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("remove a profile from a collection", |conn| {
            collections::remove_profile_from_collection(conn, collection_id, profile_name)
        })
    }

    pub fn list_profiles_in_collection(
        &self,
        collection_id: &str,
    ) -> Result<Vec<String>, MetadataStoreError> {
        self.with_conn("list profiles in a collection", |conn| {
            collections::list_profiles_in_collection(conn, collection_id)
        })
    }

    pub fn rename_collection(
        &self,
        collection_id: &str,
        new_name: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("rename a collection", |conn| {
            collections::rename_collection(conn, collection_id, new_name)
        })
    }

    pub fn update_collection_description(
        &self,
        collection_id: &str,
        description: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("update a collection description", |conn| {
            collections::update_collection_description(conn, collection_id, description)
        })
    }

    pub fn collections_for_profile(
        &self,
        profile_name: &str,
    ) -> Result<Vec<CollectionRow>, MetadataStoreError> {
        self.with_conn("list collections for a profile", |conn| {
            collections::collections_for_profile(conn, profile_name)
        })
    }

    pub fn get_collection_defaults(
        &self,
        collection_id: &str,
    ) -> Result<Option<CollectionDefaultsSection>, MetadataStoreError> {
        self.with_conn("read collection defaults", |conn| {
            collections::get_collection_defaults(conn, collection_id)
        })
    }

    pub fn set_collection_defaults(
        &self,
        collection_id: &str,
        defaults: Option<&CollectionDefaultsSection>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("write collection defaults", |conn| {
            collections::set_collection_defaults(conn, collection_id, defaults)
        })
    }

    pub fn set_profile_favorite(
        &self,
        profile_name: &str,
        favorite: bool,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("set a profile favorite", |conn| {
            collections::set_profile_favorite(conn, profile_name, favorite)
        })
    }

    pub fn list_favorite_profiles(&self) -> Result<Vec<String>, MetadataStoreError> {
        self.with_conn("list favorite profiles", |conn| {
            collections::list_favorite_profiles(conn)
        })
    }
}
