//! Host-tool runner for prefix dependency checks, installs and Windows-version
//! restoration. Every host command is lifecycle-bound (`--watch-bus` inside
//! Flatpak) so cancellation and timeouts also stop the host process.

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
pub use install::{
    install_packages, prepare_install_packages, PrefixDependencyInstall,
    PreparedPrefixDependencyInstall,
};
pub use restore::restore_prefix_windows_version;
pub(crate) use restore::restore_prefix_windows_version_with_timeout;
pub use sanitize::sanitize_output_for_ui;
pub use snapshot::is_windows_version_verb;
