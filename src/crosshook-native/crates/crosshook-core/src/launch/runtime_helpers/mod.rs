pub mod environment;
pub mod path_resolution;
pub mod platform;
pub mod proton_command;

#[cfg(test)]
mod tests;

/// Default `PATH` used when the host environment does not set `PATH`.
pub const DEFAULT_HOST_PATH: &str = "/usr/bin:/bin";
const DEFAULT_SHELL: &str = "/bin/bash";
/// Host-shell script that records its own PID at `$1`, then `exec`s the rest of argv.
///
/// Any existing path at `$1` (file, symlink, FIFO, device) is refused up front, and
/// `set -C` (noclobber) then makes the redirect an exclusive create (`O_EXCL`) for a
/// regular file appearing in between. The script never creates directories, and it
/// fails closed: when the capture is refused it exits 125 before `exec`, so gamescope
/// does not start.
pub(crate) const FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT: &str =
    "{ [ -e \"$1\" ] || [ -L \"$1\" ]; } && exit 125; \
     set -C; printf '%s' \"$$\" > \"$1\" || exit 125; shift; exec \"$@\"";

// Re-exports keep the existing `use crate::launch::runtime_helpers::X` paths resolving.

pub use environment::{
    host_environment_map, merge_optimization_and_custom_into_map, merge_runtime_proton_into_map,
};

pub use path_resolution::{
    apply_working_directory, attach_log_stdio, resolve_effective_working_directory,
    resolve_proton_paths, resolve_steam_client_install_path, resolve_wine_prefix_path,
    ResolvedProtonPaths,
};

pub use platform::{is_unshare_net_available, resolve_umu_run_path};

pub use proton_command::{
    build_direct_proton_command_with_wrappers_in_directory, build_gamescope_args,
    collect_pressure_vessel_paths,
};
// Crate-only: the script runner passes the sandbox flag explicitly.
pub(crate) use proton_command::build_proton_command_with_gamescope_pid_capture_in_directory_inner;

// `pub(crate)` items — re-export with the same visibility
pub(crate) use environment::env_value;
