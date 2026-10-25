//! Side-effect seam for Phase A, so step order is testable without a real
//! process environment.
//!
//! [`PreRuntimeHost`] abstracts every effect `pre_runtime` can perform:
//! reading the process thread count, Flatpak detection, the XDG override,
//! the two migrations, and environment-variable access. Production uses
//! [`SystemHost`]; unit tests use a recording fake, and the
//! `bootstrap_ordering` integration harness drives the real system host in a
//! re-executed child process that has no threads of its own.
//!
//! The trait and the `pre_runtime_with_host` entry point are public only
//! under the crate `test-seams` feature (and `#[doc(hidden)]`); otherwise
//! they are crate-private.

use std::ffi::{OsStr, OsString};

use super::report::FlatpakMigrationReport;

macro_rules! define_pre_runtime_host {
    ($vis:vis) => {
        /// Every side effect Phase A may perform, behind one seam.
        ///
        /// Implementations must not spawn threads: `pre_runtime` reads the
        /// thread count through this trait before any other call.
        $vis trait PreRuntimeHost: Send {
            /// Current process thread count (`Threads:` in `/proc/self/status`),
            /// or `None` when it cannot be determined.
            fn thread_count(&self) -> Option<u32>;

            /// Whether the process is running inside a Flatpak sandbox.
            fn is_flatpak(&self) -> bool;

            /// Whether the user opted into Flatpak host-XDG shared mode.
            fn host_xdg_opt_in(&self) -> bool;

            /// Applies the host-XDG environment override (shared mode only).
            fn override_xdg(&mut self);

            /// Runs the first-run Flatpak migration (isolated mode only).
            ///
            /// Errors are returned as a plain detail string; `pre_runtime`
            /// sanitizes it and records it as a degraded step, never fatal.
            fn run_flatpak_migration(&mut self) -> Result<FlatpakMigrationReport, String>;

            /// Runs the legacy Tauri app-id directory migration.
            fn migrate_legacy_app_id(&mut self);

            /// Reads one environment variable.
            fn env_var(&self, key: &OsStr) -> Option<OsString>;

            /// Sets one environment variable.
            fn set_env_var(&mut self, key: &OsStr, value: &OsStr);

            /// Removes one environment variable.
            fn remove_env_var(&mut self, key: &OsStr);
        }
    };
}

#[cfg(feature = "test-seams")]
define_pre_runtime_host!(pub);
#[cfg(not(feature = "test-seams"))]
define_pre_runtime_host!(pub(crate));

/// Production [`PreRuntimeHost`] backed by the real process environment.
///
/// The `PreRuntimeHost` implementation lives in `env_policy.rs` alongside
/// [`PreRuntimeEnv`](super::env_policy::PreRuntimeEnv); `env_policy.rs` and
/// `pre_runtime.rs` are the only files allowed to call `std::env::set_var` /
/// `remove_var` directly, and `pre_runtime` records a multi-threaded entry
/// (for example from externally preloaded libraries) as a degraded step
/// before any mutation can be reached; it does not block the mutation.
#[cfg(feature = "test-seams")]
#[derive(Debug, Default)]
pub struct SystemHost;
/// Production [`PreRuntimeHost`] backed by the real process environment.
#[cfg(not(feature = "test-seams"))]
#[derive(Debug, Default)]
pub(crate) struct SystemHost;

impl SystemHost {
    /// Creates the production host seam.
    pub fn new() -> Self {
        Self
    }
}
