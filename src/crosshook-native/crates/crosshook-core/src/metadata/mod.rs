//! SQLite metadata-DB facade.
//!
//! This module is a thin re-export surface. Business logic lives in per-domain
//! submodules; `mod.rs` itself holds only `mod …;` declarations, `pub use`
//! re-exports, and module-level documentation.
//!
//! # Layout
//!
//! The `MetadataStore` struct is defined in [`store`]. Every public method is
//! implemented on `MetadataStore` via additional `impl` blocks spread across
//! the per-domain files below. Callers keep using `crate::metadata::...`
//! paths unchanged.
//!
//! ## Core
//!
//! - [`store`] — `MetadataStore` struct, constructors, `with_conn*` helpers
//! - [`util`] — shared utilities (`in_clause_placeholders`)
//! - [`db`] — SQLite connection opening (permissions, symlink guard)
//! - [`migrations`] — schema migrations (current: **v23**)
//! - [`models`] — shared row types, error type, size limits
//!
//! ## Per-domain operations (`*_ops.rs` → delegates to `*_store.rs`)
//!
//! - [`profile_ops`] / [`profile_sync`] — profile write/rename/delete/sync
//! - [`launcher_ops`] / [`launcher_sync`] / [`launch_history`] — launcher exports, launch ops
//! - [`community_ops`] / [`community_index`] — community tap indexing, trainer search
//! - [`collections_ops`] / [`collections`] — collections, favorites, per-collection defaults
//! - [`cache_ops`] / [`cache_store`] — generic external cache (`external_cache_entries`)
//! - [`launch_queries`] — usage-insights queries (`query_most_launched`, etc.)
//! - [`health_ops`] / [`health_store`] — profile health snapshots
//! - [`game_image_ops`] / [`game_image_store`] — Steam game image cache
//! - [`offline_ops`] / [`offline_store`] — offline readiness, trainer hash cache, tap offline state
//! - [`version_ops`] / [`version_store`] — version snapshots + correlation status
//! - [`config_history_ops`] / [`config_history_store`] — TOML config revision history
//! - [`preset_ops`] / [`preset_store`] — bundled + per-profile optimization presets
//! - [`catalog_ops`] — optimization / readiness / readiness-snapshot / proton release catalogs
//! - [`prefix_ops`] / [`prefix_deps_store`] / [`prefix_storage_store`] — prefix dep state and storage snapshots
//! - [`suggestion_store`] / [`readiness_dismissal_store`] — dismissal tables (own `impl` blocks)
//!
//! ## Tests
//!
//! Tests are split into per-domain `*_tests.rs` files sharing fixtures via
//! [`test_support`]:
//!
//! - [`profile_sync_tests`], [`launcher_tests`], [`community_index_tests`]
//! - [`cache_tests`], [`collections_crud_tests`], [`collections_defaults_tests`]
//! - [`collections_favorites_tests`], [`launch_queries_tests`]
//! - [`migrations_sanity_tests`], [`trainer_hash_tests`]
//! - [`version_store_tests`], [`correlation_status_tests`]
//!
//! This layout is a refactor of a previously-3,747-line `mod.rs`; see issue #291.

mod cache_ops;
mod cache_store;
mod catalog_ops;
mod collections;
mod collections_ops;
mod community_index;
mod community_ops;
mod config_history_ops;
mod config_history_store;
mod db;
mod game_image_ops;
mod game_image_store;
mod health_ops;
mod health_store;
mod launch_history;
mod launch_queries;
mod launcher_ops;
mod launcher_sync;
mod migrations;
mod models;
mod offline_ops;
pub(crate) mod offline_store;
mod optimization_catalog_store;
mod prefix_deps_store;
mod prefix_ops;
mod prefix_storage_store;
mod preset_ops;
mod preset_store;
mod profile_ops;
pub mod profile_sync;
mod proton_catalog_store;
mod readiness_catalog_store;
mod readiness_dismissal_store;
mod readiness_snapshot_store;
mod store;
mod suggestion_store;
mod util;
mod version_ops;
mod version_store;

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod collections_crud_tests;
#[cfg(test)]
mod collections_defaults_tests;
#[cfg(test)]
mod collections_favorites_tests;
#[cfg(test)]
mod community_index_tests;
#[cfg(test)]
mod correlation_status_tests;
#[cfg(test)]
mod launch_queries_tests;
#[cfg(test)]
mod launcher_tests;
#[cfg(test)]
mod migrations_sanity_tests;
#[cfg(test)]
mod profile_sync_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod trainer_hash_tests;
#[cfg(test)]
mod version_store_tests;

pub use game_image_store::GameImageCacheRow;
pub use health_store::HealthSnapshotRow;
pub use models::{
    BundledOptimizationPresetRow, CacheEntryStatus, CollectionRow, CommunityProfileRow,
    CommunityTapRow, ConfigRevisionRow, ConfigRevisionSource, DriftState, FailureTrendRow,
    LaunchOutcome, MetadataStoreError, PrefixDependencyStateRow, PrefixStorageCleanupAuditRow,
    PrefixStorageSnapshotRow, ProfileLaunchPresetOrigin, SyncReport, SyncSource,
    VersionCorrelationStatus, VersionSnapshotRow, MAX_CACHE_PAYLOAD_BYTES,
    MAX_CONFIG_REVISIONS_PER_PROFILE, MAX_DIAGNOSTIC_JSON_BYTES, MAX_HISTORY_LIST_LIMIT,
    MAX_SNAPSHOT_TOML_BYTES, MAX_VERSION_SNAPSHOTS_PER_PROFILE,
};
pub use offline_store::{CommunityTapOfflineRow, OfflineReadinessRow, TrainerHashCacheRow};
pub use profile_sync::sha256_hex;
pub use proton_catalog_store::ProtonCatalogRow;
pub use readiness_snapshot_store::HostReadinessSnapshotRow;
pub use store::MetadataStore;
pub use version_store::{compute_correlation_status, hash_trainer_file};
