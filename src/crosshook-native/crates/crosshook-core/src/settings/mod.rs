//! Application settings persistence helpers.
pub mod recent;
pub mod webkit_import;

mod paths;
mod store;
mod types;
mod ui;

pub(crate) use paths::expand_path_with_tilde;
pub use paths::resolve_profiles_directory_from_config;
pub use recent::{RecentFilesData, RecentFilesStore, RecentFilesStoreError};
pub use store::{SettingsStore, SettingsStoreError};
pub use types::{
    clamp_config_history_max_revisions, clamp_recent_files_limit,
    config_history_max_revisions_from_settings, AppSettingsData, ConfigHistorySettings,
    HighContrastPreference, ReducedMotionPreference, UmuDatabaseLookupPreference, UmuPreference,
    CONFIG_HISTORY_MAX_REVISIONS_MAX, CONFIG_HISTORY_MAX_REVISIONS_MIN, RECENT_FILES_LIMIT_MAX,
    RECENT_FILES_LIMIT_MIN,
};
pub use ui::{
    clamp_console_drawer_height, clamp_inspector_width, clamp_ui_scale, clamp_window_height,
    clamp_window_width, is_valid_palette_id, ControllerMode, LibraryViewMode, UiDensity, UiKey,
    UiKeySet, UiRenderer, UiSettings, PALETTE_ID_MAX_BYTES, PALETTE_RECENT_MAX,
    UI_CONSOLE_DRAWER_HEIGHT_DEFAULT, UI_CONSOLE_DRAWER_HEIGHT_MAX, UI_CONSOLE_DRAWER_HEIGHT_MIN,
    UI_INSPECTOR_WIDTH_DEFAULT, UI_INSPECTOR_WIDTH_MAX, UI_INSPECTOR_WIDTH_MIN, UI_SCALE_DEFAULT,
    UI_SCALE_MAX, UI_SCALE_MIN, UI_WINDOW_HEIGHT_MAX, UI_WINDOW_HEIGHT_MIN, UI_WINDOW_WIDTH_MAX,
    UI_WINDOW_WIDTH_MIN,
};

#[cfg(test)]
mod tests;
