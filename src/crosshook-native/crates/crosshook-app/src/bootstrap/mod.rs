//! Startup bootstrap: a typed, testable ordering contract split in two phases.
//!
//! # Ordering contract
//!
//! **Phase A** ([`pre_runtime`]) is synchronous and runs before any Tokio
//! runtime, Tauri builder, GTK initialization, or thread spawned by CrossHook
//! itself, in this fixed order:
//!
//! 1. Flatpak gate (+ XDG override in host-XDG shared mode)
//! 2. First-run Flatpak migration (isolated mode only)
//! 3. Legacy Tauri app-id migration
//! 4. UI [`EnvPolicy`] hook
//!
//! Phase A returns a [`PreRuntime`] token. **Phase B**
//! (`App::bootstrap`, in `app_bootstrap`) consumes that token, so skipping
//! Phase A is a compile error. Phase B initializes stores (settings →
//! logging → remaining stores), catalogs, metadata reconciliation and nag
//! sync, then schedules startup jobs that publish `AppEvent`s; it never
//! calls `process::exit` — fatal failures are returned as [`BootstrapError`]
//! for the shell to present.
//!
//! Every step of both phases appends one record to a shared [`StepLog`]
//! (skipped steps included), so the whole startup sequence can be asserted
//! from a single ordered log. Environment mutation happens only in Phase A
//! through [`PreRuntimeEnv`]. A runtime `Threads:` check records a
//! multi-threaded entry as a degraded step (`"multi-threaded at entry"`)
//! rather than refusing to start: externally preloaded libraries (for example
//! the Steam overlay) may already own threads, and startup keeps the
//! pre-contract behavior of mutating the environment anyway.
//!
//! Ordering is verified for real, not by construction: under the
//! `crosshook-core/bootstrap-trace` feature, `platform::trace` records marks
//! and environment mutations in one process-global log, and the
//! `bootstrap_ordering` integration harness asserts rules R1–R5 against it
//! (including a deliberately misordered scenario that must fail).

mod app_bootstrap;
mod catalogs;
mod env_policy;
mod error;
#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub mod host;
#[cfg(not(feature = "test-seams"))]
mod host;
mod options;
mod pre_runtime;
mod reconcile;
mod report;
mod scans;
pub(crate) mod state;
mod steps;
mod stores;
mod ui_import;
pub(crate) mod version_scan;

pub use app_bootstrap::BootstrappedApp;
pub use env_policy::{EnvPolicy, PreRuntimeEnv};
pub use error::{BootstrapError, StoreKind};
pub use options::{BootstrapOptions, LoggingMode, RuntimeChoice, StoreRoots, UmuRefresh};
pub use pre_runtime::{pre_runtime, FlatpakMode, PreRuntime, PreRuntimeOptions};
pub use report::{
    DegradedStores, FlatpakMigrationReport, FlatpakStartup, JobSnapshotKind, JobState,
    OfflineReadinessScanSummary, OnboardingStartup, StartupJobs, StartupReport, UmuRefreshSummary,
    VersionScanSummary,
};
pub use state::StartupHandle;
pub use steps::{BootstrapStep, StepLog, StepOrderError, StepOutcome, StepRecord};
pub use stores::{Services, Stores};

pub use crate::events::StartupJobKind;

#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub use host::{PreRuntimeHost, SystemHost};
#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub use pre_runtime::{pre_runtime_misordered_with_host, pre_runtime_with_host};
