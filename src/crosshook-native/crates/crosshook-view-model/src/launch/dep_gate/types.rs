//! Data types of the dependency gate: scope, statuses, state, intents and
//! effects.

use super::LaunchAction;
use crate::ServiceError;

/// Prefix the gate checks, and what the profile requires there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GateScope {
    /// Selected profile name.
    pub profile_name: String,
    /// Effective prefix (see [`GateScope::effective_prefix_path`]); empty
    /// means "no prefix", which never blocks a launch.
    pub prefix_path: String,
    /// `trainer.required_protontricks`.
    pub required_packages: Vec<String>,
    /// `auto_install_prefix_deps` setting: install missing packages without
    /// asking.
    pub auto_install: bool,
}

impl GateScope {
    /// Effective prefix: the trimmed runtime prefix, else the trimmed Steam
    /// compatdata path, else empty (`resolveEffectivePrefixPath`).
    #[must_use]
    pub fn effective_prefix_path(runtime_prefix: &str, steam_compatdata: &str) -> String {
        [runtime_prefix, steam_compatdata]
            .iter()
            .map(|p| p.trim())
            .find(|p| !p.is_empty())
            .unwrap_or_default()
            .to_owned()
    }

    pub(super) fn same_target(&self, other: &Self) -> bool {
        self.profile_name == other.profile_name && self.prefix_path == other.prefix_path
    }
}

/// Windows-version restore state of a prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestoreState {
    /// Nothing to restore.
    NotRequired,
    /// The restore has not completed.
    Pending,
    /// Restored.
    Succeeded,
    /// The restore failed.
    Failed,
}

/// Repair status of a prefix (core `PrefixVersionRepairStatus`, minus the
/// recovery report the gate does not use).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepairStatus {
    /// A restore journal row exists, so launches stay blocked.
    pub required: bool,
    /// `Pending` or `Failed` while a row exists.
    pub state: Option<RestoreState>,
    /// Last restore error.
    pub last_error: Option<String>,
    /// A job holds the prefix.
    pub in_progress: bool,
}

impl RepairStatus {
    /// A job holds the prefix over a provisional repair row: wait and retry
    /// rather than repair (`isPrefixBusy`).
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.in_progress && self.required
    }

    pub(super) fn failed(message: String) -> Self {
        Self {
            required: true,
            state: Some(RestoreState::Failed),
            last_error: Some(message),
            in_progress: false,
        }
    }
}

/// State of one prefix dependency package (core `DependencyState`). The gate
/// blocks on `Unknown`, `Missing` and `InstallFailed` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyState {
    /// Never checked; treated as missing.
    Unknown,
    /// Installed in the prefix.
    Installed,
    /// Checked and not installed.
    Missing,
    /// The last install attempt failed; treated as missing.
    InstallFailed,
    /// The check itself failed (tool unavailable, error); does not block.
    CheckFailed,
    /// The user chose to skip the package; does not block.
    UserSkipped,
}

impl DependencyState {
    /// Whether the gate treats the package as missing. `CheckFailed` and
    /// `UserSkipped` do not block (Classic parity).
    #[must_use]
    pub const fn blocks_launch(self) -> bool {
        matches!(self, Self::Unknown | Self::Missing | Self::InstallFailed)
    }
}

/// Status of one installed-or-not package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyStatus {
    /// Package (winetricks verb).
    pub package_name: String,
    /// State.
    pub state: DependencyState,
}

/// `prefix-dep-complete` event of an install job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCompletion {
    /// Profile the install ran for.
    pub profile_name: String,
    /// Prefix the install ran in.
    pub prefix_path: String,
    /// Whether the packages installed.
    pub install_succeeded: bool,
    /// Outcome of the Windows-version restore that follows the install.
    pub restore_state: RestoreState,
    /// Restore error, if any.
    pub restore_error: Option<String>,
}

/// Id of one gate operation; results carrying any other id are ignored.
///
/// Unique across gate instances in the process (each gate gets its own
/// `mount`), so a reply meant for an earlier gate never matches a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OpId {
    pub(super) mount: u64,
    pub(super) seq: u64,
}

/// What the gate shows and waits for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GateState {
    /// No launch is pending.
    #[default]
    Idle,
    /// Checking the prefix before launching `action`.
    Checking {
        /// Pending launch.
        action: LaunchAction,
    },
    /// Packages are missing; the user chooses install, skip or cancel.
    Missing {
        /// Pending launch.
        action: LaunchAction,
        /// Missing packages, in the profile's order.
        packages: Vec<String>,
    },
    /// Installing `packages`, then launching `action`.
    Installing {
        /// Pending launch.
        action: LaunchAction,
        /// Packages being installed.
        packages: Vec<String>,
        /// Started without asking (`auto_install`).
        auto: bool,
        /// The install finished and the prefix repair status is being
        /// re-verified.
        verifying: bool,
    },
    /// The prefix needs a Windows-version repair before any launch.
    RepairRequired {
        /// Pending launch.
        action: LaunchAction,
        /// Status shown (last error).
        status: RepairStatus,
        /// A repair is running.
        repairing: bool,
    },
    /// A job holds the prefix; retry when it is done.
    Busy {
        /// Pending launch.
        action: LaunchAction,
        /// Status shown.
        status: RepairStatus,
    },
}

/// User actions and service results the gate accepts.
#[derive(Debug, Clone, PartialEq)]
pub enum GateIntent {
    /// The selected profile, prefix, packages or setting changed. A different
    /// profile or prefix resets the gate and makes pending results stale.
    ScopeChanged(GateScope),
    /// The user pressed launch. Ignored while an install or repair runs.
    RequestLaunch(LaunchAction),
    /// Result of [`GateEffect::CheckRepairStatus`].
    RepairStatusLoaded {
        /// Operation.
        op: OpId,
        /// Status or error.
        result: Result<RepairStatus, ServiceError>,
    },
    /// Result of [`GateEffect::CheckDependencies`].
    DependenciesLoaded {
        /// Operation.
        op: OpId,
        /// Package statuses or error.
        result: Result<Vec<DependencyStatus>, ServiceError>,
    },
    /// "Install + Launch" in the missing-packages modal.
    InstallDependencies,
    /// The install request returned (the job runs on; its end is
    /// [`GateIntent::InstallCompleted`]).
    InstallRequestFinished {
        /// Operation.
        op: OpId,
        /// Whether the job started.
        result: Result<(), ServiceError>,
    },
    /// An install job finished (any profile or prefix; others are ignored).
    InstallCompleted(InstallCompletion),
    /// "Skip and Launch" in the missing-packages modal.
    Skip,
    /// "Cancel" in any gate modal.
    Cancel,
    /// "Try Again" in the busy modal.
    Retry,
    /// "Repair + Launch" in the repair modal.
    Repair,
    /// Result of [`GateEffect::RepairPrefixVersion`].
    RepairFinished {
        /// Operation.
        op: OpId,
        /// Status after the repair, or error.
        result: Result<RepairStatus, ServiceError>,
    },
}

/// Service calls the bridge runs for the gate, and the launch go-ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateEffect {
    /// Read the prefix repair status; answer with
    /// [`GateIntent::RepairStatusLoaded`].
    CheckRepairStatus {
        /// Operation.
        op: OpId,
        /// Profile.
        profile_name: String,
        /// Prefix.
        prefix_path: String,
    },
    /// Read the dependency status; answer with
    /// [`GateIntent::DependenciesLoaded`].
    CheckDependencies {
        /// Operation.
        op: OpId,
        /// Profile.
        profile_name: String,
        /// Prefix.
        prefix_path: String,
    },
    /// Start a dependency install; answer with
    /// [`GateIntent::InstallRequestFinished`], then forward its
    /// `prefix-dep-complete` event as [`GateIntent::InstallCompleted`].
    ///
    /// Subscribe to `prefix-dep-complete` **before** starting the install:
    /// the job can finish before the install request returns. The event
    /// carries no job id, so any completion for the same profile and prefix
    /// counts (Classic parity).
    InstallDependencies {
        /// Operation.
        op: OpId,
        /// Profile.
        profile_name: String,
        /// Prefix.
        prefix_path: String,
        /// Packages.
        packages: Vec<String>,
    },
    /// Repair the prefix Windows version; answer with
    /// [`GateIntent::RepairFinished`].
    RepairPrefixVersion {
        /// Operation.
        op: OpId,
        /// Profile.
        profile_name: String,
        /// Prefix.
        prefix_path: String,
    },
    /// The launch may start now.
    Launch(LaunchAction),
}
