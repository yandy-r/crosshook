//! Host-tool runner for prefix dependency checks, installs and Windows-version
//! restoration. Every host command is lifecycle-bound (`--watch-bus` inside
//! Flatpak) so cancellation and timeouts also stop the host process.

mod bounded;
mod check;
mod command;
mod install;
mod restore;
mod sanitize;
mod snapshot;

#[cfg(test)]
mod tests;

pub use check::check_installed;
pub use command::normalize_prefix_path;
pub use install::{prepare_install_packages, PreparedPrefixDependencyInstall};
pub use restore::restore_prefix_windows_version;
pub use sanitize::sanitize_output_for_ui;
pub use snapshot::is_windows_version_verb;
