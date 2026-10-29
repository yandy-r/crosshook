//! Runtime, persistence roots and optional startup work.

use std::path::PathBuf;
use tokio::runtime::Handle;

/// Runtime used for application jobs.
pub enum RuntimeChoice {
    /// Build and own an application runtime.
    Owned,
    /// Borrow the shell's runtime without shutting it down.
    Handle(Handle),
}

/// Filesystem roots for persistent stores.
#[derive(Debug, Clone)]
pub enum StoreRoots {
    /// Resolve the user's XDG directories after Phase A.
    Xdg,
    /// Explicit roots for portable usage and isolated tests.
    Explicit {
        /// CrossHook configuration directory.
        config_dir: PathBuf,
        /// CrossHook metadata/data directory.
        data_dir: PathBuf,
    },
}

/// Process-global logging initialization policy.
#[derive(Debug, Clone, Copy)]
pub enum LoggingMode {
    /// Initialize logging with the user's filter.
    Init {
        /// Also mirror file logging to standard output.
        mirror_stdout: bool,
    },
    /// Leave the process subscriber unchanged.
    Skip,
}

/// Policy for the optional network-backed compatibility refresh.
#[derive(Debug, Clone, Copy)]
pub enum UmuRefresh {
    /// Refresh the database in a background job.
    Enabled,
    /// Skip refreshing, retaining an explicit explanation in the report.
    Skip {
        /// Why the shell disabled refresh.
        reason: &'static str,
    },
}

/// Policy for the startup prefix Windows-version journal recovery job.
#[derive(Debug, Clone, Copy)]
pub enum PrefixJournalRecovery {
    /// Retry pending journal rows in a background job.
    Enabled,
    /// Skip recovery, logging an explicit explanation.
    Skip {
        /// Why the shell disabled recovery.
        reason: &'static str,
    },
}

/// Phase-B choices. Phase A must finish before any runtime is constructed.
#[non_exhaustive]
pub struct BootstrapOptions {
    /// Runtime ownership.
    pub runtime: RuntimeChoice,
    /// Persistent store roots.
    pub store_roots: StoreRoots,
    /// Logging policy.
    pub logging: LoggingMode,
    /// Compatibility database refresh policy.
    pub umu_refresh: UmuRefresh,
    /// Startup prefix journal recovery policy.
    pub prefix_journal_recovery: PrefixJournalRecovery,
}

impl BootstrapOptions {
    /// Production defaults with an owned runtime and XDG stores.
    pub fn new() -> Self {
        Self {
            runtime: RuntimeChoice::Owned,
            store_roots: StoreRoots::Xdg,
            logging: LoggingMode::Init {
                mirror_stdout: false,
            },
            umu_refresh: UmuRefresh::Enabled,
            prefix_journal_recovery: PrefixJournalRecovery::Enabled,
        }
    }

    /// Reuses an existing runtime, created after Phase A.
    pub fn with_runtime_handle(mut self, handle: Handle) -> Self {
        self.runtime = RuntimeChoice::Handle(handle);
        self
    }

    /// Selects filesystem roots.
    pub fn with_store_roots(mut self, roots: StoreRoots) -> Self {
        self.store_roots = roots;
        self
    }

    /// Selects logging initialization.
    pub fn with_logging(mut self, logging: LoggingMode) -> Self {
        self.logging = logging;
        self
    }

    /// Selects compatibility refresh policy.
    pub fn with_umu_refresh(mut self, refresh: UmuRefresh) -> Self {
        self.umu_refresh = refresh;
        self
    }

    /// Selects startup prefix journal recovery policy.
    pub fn with_prefix_journal_recovery(mut self, recovery: PrefixJournalRecovery) -> Self {
        self.prefix_journal_recovery = recovery;
        self
    }
}

impl Default for BootstrapOptions {
    fn default() -> Self {
        Self::new()
    }
}
