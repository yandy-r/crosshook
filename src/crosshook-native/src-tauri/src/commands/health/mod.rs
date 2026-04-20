mod batch;
mod enrich;
mod prefetch;
mod single;
mod snapshots;
mod steam;
mod types;

pub use batch::{
    __cmd__batch_validate_profiles, batch_validate_profiles, build_enriched_health_summary,
};
pub use single::{__cmd__get_profile_health, get_profile_health};
pub use snapshots::{
    __cmd__get_cached_health_snapshots, __cmd__get_cached_offline_readiness_snapshots,
    get_cached_health_snapshots, get_cached_offline_readiness_snapshots,
};
#[allow(unused_imports)]
pub use types::{
    CachedHealthSnapshot, CachedOfflineReadinessSnapshot, EnrichedHealthSummary,
    EnrichedProfileHealthReport, OfflineReadinessBrief, ProfileHealthMetadata,
};

pub(super) const FAILURE_TREND_WINDOW_DAYS: u32 = 30;
