mod content;
mod names;
mod paths;
mod preview;
mod service;
mod types;

pub use names::sanitize_launcher_slug;
pub use paths::resolve_target_home_path;
pub use preview::{preview_desktop_entry_content, preview_trainer_script_content};
pub use service::export_launchers;
pub use types::{
    validate, SteamExternalLauncherExportError, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult, SteamExternalLauncherExportValidationError,
};

pub(crate) use content::{build_desktop_entry_content, build_trainer_script_content};
pub(crate) use names::{resolve_display_name, strip_trainer_suffix};
pub(crate) use paths::{combine_host_unix_path, write_host_text_file};

#[cfg(test)]
mod tests;
