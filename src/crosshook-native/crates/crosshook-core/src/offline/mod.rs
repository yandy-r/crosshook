//! Offline trainer classification, readiness scoring, and hash caching.

pub mod hash;
pub mod network;
pub mod readiness;
pub mod trainer_type;

pub use hash::{
    normalize_sha256_hex, trainer_hash_launch_check, verify_and_cache_trainer_hash,
    HashVerifyResult, TrainerHashBaselineResult, TrainerHashCommunityAdvisory,
    TrainerHashLaunchOutcome,
};
pub use network::is_network_available;
pub use readiness::{
    check_offline_preflight, compute_offline_readiness, enrich_health_report_with_offline,
    persist_offline_readiness_from_report, OfflineReadinessPersistHints, OfflineReadinessReport,
};
pub use trainer_type::{
    global_trainer_type_catalog, initialize_trainer_type_catalog, load_trainer_type_catalog,
    OfflineCapability, TrainerTypeCatalog, TrainerTypeEntry,
};
