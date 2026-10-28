//! Injectable umu database refresh boundary.

use crosshook_core::umu_database::{self, UmuDatabaseRefreshStatus};

use crate::services::BoxFuture;

/// Refreshes the umu database without coupling callers to the core HTTP client.
pub trait UmuRefresher: Send + Sync {
    /// Refreshes the cached database and reports its resulting cache status.
    fn refresh(&self) -> BoxFuture<'_, Result<UmuDatabaseRefreshStatus, umu_database::Error>>;
}

pub(super) struct CoreUmuRefresher;

impl UmuRefresher for CoreUmuRefresher {
    fn refresh(&self) -> BoxFuture<'_, Result<UmuDatabaseRefreshStatus, umu_database::Error>> {
        Box::pin(umu_database::refresh_umu_database())
    }
}
