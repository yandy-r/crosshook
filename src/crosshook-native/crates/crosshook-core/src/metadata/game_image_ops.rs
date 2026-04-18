use super::{game_image_store, GameImageCacheRow, MetadataStore, MetadataStoreError};

impl MetadataStore {
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_game_image(
        &self,
        steam_app_id: &str,
        image_type: &str,
        source: &str,
        file_path: &str,
        file_size: Option<i64>,
        content_hash: Option<&str>,
        mime_type: Option<&str>,
        source_url: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert a game image cache entry", |conn| {
            game_image_store::upsert_game_image(
                conn,
                steam_app_id,
                image_type,
                source,
                file_path,
                file_size,
                content_hash,
                mime_type,
                source_url,
                expires_at,
            )
        })
    }

    pub fn get_game_image(
        &self,
        steam_app_id: &str,
        image_type: &str,
    ) -> Result<Option<GameImageCacheRow>, MetadataStoreError> {
        self.with_conn("get a game image cache entry", |conn| {
            game_image_store::get_game_image(conn, steam_app_id, image_type)
        })
    }

    pub fn evict_expired_images(&self) -> Result<Vec<String>, MetadataStoreError> {
        self.with_conn_mut("evict expired game image cache entries", |conn| {
            game_image_store::evict_expired_images(conn)
        })
    }
}
