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
    clamp_recent_files_limit, AppSettingsData, UmuPreference, RECENT_FILES_LIMIT_MAX,
    RECENT_FILES_LIMIT_MIN,
};

#[cfg(test)]
mod tests;
