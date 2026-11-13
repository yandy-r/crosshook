pub mod environment;
pub mod path_resolution;
pub mod platform;
pub mod proton_command;

#[cfg(test)]
mod tests;

/// Default `PATH` used when the host environment does not set `PATH`.
pub const DEFAULT_HOST_PATH: &str = "/usr/bin:/bin";
const DEFAULT_SHELL: &str = "/bin/bash";
const FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT: &str =
    "capture_dir=$(dirname -- \"$1\"); mkdir -p -- \"$capture_dir\"; printf '%s' \"$$\" > \"$1\"; shift; exec \"$@\"";

// Re-exports keep the existing `use crate::launch::runtime_helpers::X` paths resolving.

pub use environment::{
    host_environment_map, merge_optimization_and_custom_into_map, merge_runtime_proton_into_map,
};

pub use path_resolution::{
    apply_working_directory, attach_log_stdio, resolve_effective_working_directory,
    resolve_proton_paths, resolve_steam_client_install_path, resolve_wine_prefix_path,
    ResolvedProtonPaths,
};

pub use platform::{
    is_unshare_net_available, launch_platform_capabilities, resolve_umu_run_path,
    LaunchPlatformCapabilities,
};

pub use proton_command::{
    build_direct_proton_command_with_wrappers_in_directory, build_gamescope_args,
    build_proton_command_with_gamescope_in_directory,
    build_proton_command_with_gamescope_pid_capture_in_directory, collect_pressure_vessel_paths,
};

// `pub(crate)` items — re-export with the same visibility
pub(crate) use environment::env_value;
