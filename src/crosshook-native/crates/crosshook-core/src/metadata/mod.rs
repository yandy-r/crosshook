//! SQLite metadata-DB facade.
//!
//! This module is a thin re-export surface. Business logic lives in per-domain
//! submodules (`store.rs`, `*_ops.rs`, `launch_queries.rs`, `*_store.rs`). Tests
//! are split into per-domain `*_tests.rs` files that share fixtures via
//! `test_support.rs`.

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
