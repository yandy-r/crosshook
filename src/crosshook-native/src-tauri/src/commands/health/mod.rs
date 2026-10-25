mod batch;
mod single;
mod snapshots;

pub use batch::batch_validate_profiles;
pub use single::get_profile_health;
pub use snapshots::{get_cached_health_snapshots, get_cached_offline_readiness_snapshots};

// Re-export Tauri command macros so `generate_handler!` can resolve `commands::health::<name>`.
pub use batch::__cmd__batch_validate_profiles;
pub use batch::__tauri_command_name_batch_validate_profiles;
pub use single::__cmd__get_profile_health;
pub use single::__tauri_command_name_get_profile_health;
pub use snapshots::__cmd__get_cached_health_snapshots;
pub use snapshots::__cmd__get_cached_offline_readiness_snapshots;
pub use snapshots::__tauri_command_name_get_cached_health_snapshots;
pub use snapshots::__tauri_command_name_get_cached_offline_readiness_snapshots;

#[cfg(test)]
mod tests;
