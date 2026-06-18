//! Application settings persistence helpers.
pub mod recent;

mod paths;
mod store;
mod types;

pub(crate) use paths::expand_path_with_tilde;
pub use paths::resolve_profiles_directory_from_config;
pub use recent::{RecentFilesData, RecentFilesStore, RecentFilesStoreError};
pub use store::{SettingsStore, SettingsStoreError};
pub use types::{
    clamp_config_history_max_revisions, clamp_recent_files_limit,
    config_history_max_revisions_from_settings, AppSettingsData, ConfigHistorySettings,
    UmuDatabaseLookupPreference, UmuPreference, CONFIG_HISTORY_MAX_REVISIONS_MAX,
    CONFIG_HISTORY_MAX_REVISIONS_MIN, RECENT_FILES_LIMIT_MAX, RECENT_FILES_LIMIT_MIN,
};

#[cfg(test)]
mod tests;
