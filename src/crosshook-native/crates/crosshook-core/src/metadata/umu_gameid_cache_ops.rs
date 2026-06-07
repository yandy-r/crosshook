use super::{umu_gameid_cache_store, MetadataStore, MetadataStoreError, UmuGameIdCacheRow};

impl MetadataStore {
    pub fn put_umu_gameid_cache_entry(
        &self,
        row: &UmuGameIdCacheRow,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("put umu GAMEID cache entry", |conn| {
            umu_gameid_cache_store::put_umu_gameid_cache_entry(conn, row)
        })
    }

    pub fn get_umu_gameid_cache_entry(
        &self,
        store: &str,
        codename: &str,
    ) -> Result<Option<UmuGameIdCacheRow>, MetadataStoreError> {
        self.with_conn("get umu GAMEID cache entry", |conn| {
            umu_gameid_cache_store::get_umu_gameid_cache_entry(conn, store, codename)
        })
    }

    pub fn get_stale_umu_gameid_cache_entry(
        &self,
        store: &str,
        codename: &str,
    ) -> Result<Option<UmuGameIdCacheRow>, MetadataStoreError> {
        self.with_conn("get stale umu GAMEID cache entry", |conn| {
            umu_gameid_cache_store::get_stale_umu_gameid_cache_entry(conn, store, codename)
        })
    }

    pub fn clear_umu_gameid_cache(&self) -> Result<usize, MetadataStoreError> {
        self.with_conn("clear umu GAMEID cache", |conn| {
            umu_gameid_cache_store::clear_umu_gameid_cache(conn)
        })
    }
}
