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
    /// CSV is readable but the app id has no matching row — umu will apply
    /// global defaults (and overwrite STEAM_COMPAT_APP_ID with a prefix MD5
    /// per umu/umu_run.py:515 verified 2026-04-14, which can break per-Proton-build
    /// local fixes — see issue #262 Witcher 3 / proton-cachyos).
    Missing,
    /// CSV source not reachable — coverage cannot be determined.
    Unknown,
}

pub use client::{refresh_umu_database, Error, UmuDatabaseRefreshStatus};
pub use coverage::check_coverage;
pub use paths::resolve_umu_database_path;
