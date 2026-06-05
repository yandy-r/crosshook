mod batch;
mod enrich;
mod prefetch;
mod single;
mod snapshots;
mod steam;
mod types;

pub use batch::{batch_validate_profiles, build_enriched_health_summary};
pub use single::get_profile_health;
pub use snapshots::{get_cached_health_snapshots, get_cached_offline_readiness_snapshots};
#[allow(unused_imports)]
pub use types::{
    CachedHealthSnapshot, CachedOfflineReadinessSnapshot, EnrichedHealthSummary,
    EnrichedProfileHealthReport, OfflineReadinessBrief, ProfileHealthMetadata,
};

// Re-export Tauri command macros so `generate_handler!` can resolve `commands::health::<name>`.
pub use batch::__cmd__batch_validate_profiles;
pub use batch::__tauri_command_name_batch_validate_profiles;
pub use single::__cmd__get_profile_health;
pub use single::__tauri_command_name_get_profile_health;
pub use snapshots::__cmd__get_cached_health_snapshots;
pub use snapshots::__cmd__get_cached_offline_readiness_snapshots;
pub use snapshots::__tauri_command_name_get_cached_health_snapshots;
pub use snapshots::__tauri_command_name_get_cached_offline_readiness_snapshots;

pub(super) const FAILURE_TREND_WINDOW_DAYS: u32 = 30;
