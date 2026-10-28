use crosshook_core::umu_database::{self, CsvCoverage};

use super::UmuService;
use crate::services::run_blocking;
use crate::AppError;

impl UmuService {
    /// Looks up Steam title coverage in the cached CSV without a network request.
    ///
    /// Missing or whitespace-only app IDs have unknown coverage.
    pub async fn coverage(&self, app_id: Option<String>) -> Result<CsvCoverage, AppError> {
        let Some(app_id) = app_id else {
            return Ok(CsvCoverage::Unknown);
        };
        if app_id.trim().is_empty() {
            return Ok(CsvCoverage::Unknown);
        }
        run_blocking(move || Ok(umu_database::check_coverage(app_id.trim(), Some("steam")))).await
    }

    /// Clears cached GAMEID lookup hits and misses, returning the deleted row count.
    pub async fn clear_gameid_cache(&self) -> Result<usize, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || metadata.clear_umu_gameid_cache().map_err(AppError::from)).await
    }
}
