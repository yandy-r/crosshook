//! umu-database CSV coverage module.
//!
//! Provides CSV-based coverage lookup for umu-launcher's protonfix database,
//! an HTTP cache refresh client, and path resolution across multiple system
//! install locations.

pub mod client;
pub mod coverage;
pub mod paths;

/// Relative path under `data_local_dir()` where CrossHook caches the umu-database CSV.
///
/// Shared between [`client`] (write path) and [`paths`] (read / resolution path) so
/// the two never drift out of sync.
pub(crate) const CROSSHOOK_UMU_DATABASE_CSV_SUBPATH: &str = "crosshook/umu-database.csv";

use serde::{Deserialize, Serialize};

/// Result of looking up a Steam app id in umu-launcher's protonfix CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvCoverage {
    /// CSV is readable and the app id has a matching row.
    Found,
    /// CSV is readable but the app id has no matching row.
    ///
    /// The upstream `umu-database` only lists titles that need protonfixes; it is
    /// not a complete catalog. Missing is the expected state for most Steam titles
    /// that still work fine with umu's global defaults.
    Missing,
    /// CSV source not reachable — coverage cannot be determined.
    Unknown,
}

pub use client::{refresh_umu_database, Error, UmuDatabaseRefreshStatus};
pub use coverage::check_coverage;
pub use paths::resolve_umu_database_path;
