use crosshook_core::protondb::{lookup_protondb, ProtonDbLookupResult};

use super::ProtonDbService;
use crate::services::run_blocking;
use crate::AppError;

impl ProtonDbService {
    /// Looks up ProtonDB data, retaining core's six-hour cache and stale fallback.
    pub async fn lookup(
        &self,
        app_id: String,
        force_refresh: bool,
    ) -> Result<ProtonDbLookupResult, AppError> {
        let metadata = self.metadata.clone();
        let runtime = tokio::runtime::Handle::current();
        run_blocking(move || {
            Ok(runtime.block_on(lookup_protondb(&metadata, &app_id, force_refresh)))
        })
        .await
    }
}
