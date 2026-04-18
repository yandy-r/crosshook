use super::{cache_store, MetadataStore, MetadataStoreError};

impl MetadataStore {
    pub fn get_cache_entry(&self, cache_key: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("get a cache entry", |conn| {
            cache_store::get_cache_entry(conn, cache_key)
        })
    }

    pub fn put_cache_entry(
        &self,
        source_url: &str,
        cache_key: &str,
        payload: &str,
        expires_at: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("put a cache entry", |conn| {
            cache_store::put_cache_entry(conn, source_url, cache_key, payload, expires_at)
        })
    }

    pub fn evict_expired_cache_entries(&self) -> Result<usize, MetadataStoreError> {
        self.with_conn("evict expired cache entries", |conn| {
            cache_store::evict_expired_cache_entries(conn)
        })
    }
}
