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

mod backup;
mod cache_ops;
mod cache_store;
mod catalog_ops;
mod collections;
mod collections_ops;
mod community_index;
mod community_ops;
pub(crate) mod config_history_hmac;
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
mod mods_ops;
mod mods_store;
mod offline_ops;
pub(crate) mod offline_store;
mod optimization_catalog_store;
mod prefix_deps_store;
mod prefix_ops;
mod prefix_storage_store;
mod prefix_version_restore_store;
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
mod umu_gameid_cache_ops;
mod umu_gameid_cache_store;
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
mod mods_launch_tests;
#[cfg(test)]
mod mods_tests;
#[cfg(test)]
mod profile_sync_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod trainer_hash_tests;
#[cfg(test)]
mod version_store_tests;

pub use community_index::degraded_catalog_from_taps;
pub use config_history_hmac::KEY_FILE_NAME as CONFIG_HISTORY_KEY_FILE_NAME;
pub use game_image_store::GameImageCacheRow;
pub use health_store::HealthSnapshotRow;
pub use models::{
    BundledOptimizationPresetRow, CacheEntryStatus, CollectionRow, CommunityProfileRow,
    ConfigRevisionRow, ConfigRevisionSource, DriftState, FailureTrendRow, HashOnlyReason,
    IntegrityFailure, LaunchHistoryEntry, LaunchOutcome, MetadataStatus, MetadataStoreError,
    PrefixDependencyStateRow, PrefixStorageCleanupAuditRow, PrefixStorageSnapshotRow,
    PrefixVersionRestoreJournalRow, ProfileLaunchPresetOrigin, RevisionIntegrity, SyncReport,
    SyncSource, VersionCorrelationStatus, VersionSnapshotRow, MAX_CACHE_PAYLOAD_BYTES,
    MAX_CONFIG_REVISIONS_PER_PROFILE, MAX_DIAGNOSTIC_JSON_BYTES, MAX_HISTORY_LIST_LIMIT,
    MAX_SNAPSHOT_TOML_BYTES, MAX_VERSION_SNAPSHOTS_PER_PROFILE,
};
pub use offline_store::{CommunityTapOfflineRow, OfflineReadinessRow, TrainerHashCacheRow};
pub use profile_sync::sha256_hex;
pub use proton_catalog_store::ProtonCatalogRow;
pub use readiness_snapshot_store::HostReadinessSnapshotRow;
pub use store::MetadataStore;
pub use umu_gameid_cache_store::UmuGameIdCacheRow;
pub(crate) use umu_gameid_cache_store::{
    normalize_umu_gameid_codename, normalize_umu_gameid_store,
};
pub use version_store::{compute_correlation_status, hash_trainer_file};
