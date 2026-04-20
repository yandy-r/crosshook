//! Runtime platform detection for Flatpak sandboxing.
//!
//! CrossHook runs both as a native Linux binary (AppImage, dev build) and
//! inside a Flatpak sandbox. Several subsystems need to know which of the two
//! environments they are running in so they can adjust process spawning and
//! resource path resolution. This module remains the single source of truth
//! for that decision while delegating the implementation to focused
//! submodules.

mod detect;
mod env;
mod gateway;
mod host_fs;
mod steam_deck;
mod xdg;

pub use detect::{is_flatpak, normalize_flatpak_host_path};
pub use gateway::{
    host_command, host_command_exists, host_command_with_env, host_command_with_env_and_directory,
    host_std_command, host_std_command_with_env,
};
pub use host_fs::{
    host_path_is_dir, host_path_is_executable_file, host_path_is_file, host_read_dir_names,
    host_read_file_bytes_if_system_path, is_allowed_host_system_compat_listing_path,
    normalized_path_exists_on_host, normalized_path_is_dir, normalized_path_is_dir_on_host,
    normalized_path_is_executable_file, normalized_path_is_executable_file_on_host,
    normalized_path_is_file, normalized_path_is_file_on_host,
};
pub use steam_deck::is_steam_deck;
pub use xdg::override_xdg_for_flatpak_host_access;

pub(crate) use env::{EnvSink, SystemEnv};
pub(crate) use gateway::{
    host_command_with_env_and_directory_inner, is_safe_host_path_lookup_name,
};
pub(crate) use steam_deck::read_host_os_release_body;

/// Flatpak desktop-portal contracts (GameMode PID registration, Background
/// watchdog protection). Additive to ADR-0001; see
/// `docs/architecture/adr-0002-flatpak-portal-contracts.md`.
pub mod portals;

#[cfg(test)]
pub(crate) mod tests;
