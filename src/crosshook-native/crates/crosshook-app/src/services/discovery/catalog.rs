use crosshook_core::discovery::{CatalogPage, CatalogQuery};
use crosshook_core::metadata::degraded_catalog_from_taps;

use super::DiscoveryService;
use crate::services::run_blocking;
use crate::AppError;

impl DiscoveryService {
    /// Queries the indexed catalog, falling back to taps on any metadata error.
    pub async fn catalog(&self, query: CatalogQuery) -> Result<CatalogPage, AppError> {
        let service = self.clone();
        run_blocking(move || match service.metadata.query_community_catalog(&query) {
            Ok(page) => Ok(page),
            Err(db_error) => {
                tracing::warn!(%db_error, "metadata catalog query failed; falling back to tap index");
                let taps = service.settings.load()?.community_taps;
                Ok(degraded_catalog_from_taps(&service.taps, &taps, &query))
            }
        }).await
    }
}
