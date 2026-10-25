//! Ordered, recorded bootstrap steps shared by Phase A and Phase B.
//!
//! Every step runs (or is explicitly skipped) in a fixed order and appends
//! exactly one [`StepRecord`], so tests and diagnostics can assert on the
//! sequence instead of on side effects alone.

use std::fmt;

/// One named bootstrap step, in canonical execution order.
///
/// Phase A steps come first ([`BootstrapStep::FlatpakGate`] through
/// [`BootstrapStep::EnvPolicy`]); Phase B continues the same log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BootstrapStep {
    /// Flatpak gate: decide native / isolated / host-XDG-shared mode and
    /// apply the XDG override when the user opted into shared mode.
    FlatpakGate,
    /// First-run Flatpak migration (isolated mode only).
    FlatpakMigration,
    /// Legacy Tauri app-id directory migration.
    LegacyAppIdMigration,
    /// Shell-supplied environment policy hook (UI renderer variables).
    EnvPolicy,
    /// Application runtime construction (Phase B).
    Runtime,
    /// Settings store initialization and load (Phase B).
    SettingsStore,
    /// Logging initialization (Phase B).
    Logging,
    /// Development helper scripts made executable (Phase B).
    DevelopmentScripts,
    /// Profile store initialization (Phase B).
    ProfileStore,
    /// Recent-files store initialization (Phase B).
    RecentFilesStore,
    /// Community tap store initialization (Phase B).
    CommunityTapStore,
    /// Metadata store initialization (Phase B; degraded, never fatal).
    MetadataStore,
    /// Optimization / readiness catalog initialization (Phase B).
    Catalogs,
    /// Metadata reconciliation and catalog persistence (Phase B).
    MetadataReconciliation,
    /// Readiness nag dismissal sync (Phase B).
    NagSync,
    /// Startup background jobs scheduled (Phase B).
    StartupJobs,
}

impl BootstrapStep {
    /// The four Phase A steps in their canonical order.
    pub const PRE_RUNTIME_ORDER: [BootstrapStep; 4] = [
        BootstrapStep::FlatpakGate,
        BootstrapStep::FlatpakMigration,
        BootstrapStep::LegacyAppIdMigration,
        BootstrapStep::EnvPolicy,
    ];

    /// Stable lowercase name used in trace marks and diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::FlatpakGate => "gate",
            Self::FlatpakMigration => "flatpak_migration",
            Self::LegacyAppIdMigration => "legacy_app_id",
            Self::EnvPolicy => "env_policy",
            Self::Runtime => "runtime",
            Self::SettingsStore => "settings_store",
            Self::Logging => "logging",
            Self::DevelopmentScripts => "development_scripts",
            Self::ProfileStore => "profile_store",
            Self::RecentFilesStore => "recent_files_store",
            Self::CommunityTapStore => "community_tap_store",
            Self::MetadataStore => "metadata_store",
            Self::Catalogs => "catalogs",
            Self::MetadataReconciliation => "metadata_reconciliation",
            Self::NagSync => "nag_sync",
            Self::StartupJobs => "startup_jobs",
        }
    }
}

impl fmt::Display for BootstrapStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How a single bootstrap step finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step performed its work.
    Ran,
    /// The step was not applicable in this mode; the reason is static text.
    Skipped(&'static str),
    /// The step failed but startup continues in a degraded mode.
    Degraded(&'static str),
}

/// One recorded bootstrap step and its outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepRecord {
    /// The step that ran.
    pub step: BootstrapStep,
    /// How the step finished.
    pub outcome: StepOutcome,
}

impl StepRecord {
    /// Creates one step record.
    pub fn new(step: BootstrapStep, outcome: StepOutcome) -> Self {
        Self { step, outcome }
    }
}

/// Ordered log of every bootstrap step outcome, across both phases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepLog {
    records: Vec<StepRecord>,
}

impl StepLog {
    /// Creates an empty step log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one step outcome at the end of the log.
    pub fn push(&mut self, step: BootstrapStep, outcome: StepOutcome) {
        self.records.push(StepRecord::new(step, outcome));
    }

    /// Appends an already-built step record.
    pub fn push_record(&mut self, record: StepRecord) {
        self.records.push(record);
    }

    /// All records in execution order.
    pub fn records(&self) -> &[StepRecord] {
        &self.records
    }

    /// Number of recorded steps.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether no step has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Verifies the Phase A prefix of the log matches the canonical order.
    ///
    /// This is the shared checker used by the unit tests and by the
    /// `bootstrap_ordering` integration harness: a deliberately misordered
    /// execution must fail here. Phase B records appended after the Phase A
    /// prefix do not affect the check.
    pub fn verify_pre_runtime_order(&self) -> Result<(), StepOrderError> {
        for (index, expected) in BootstrapStep::PRE_RUNTIME_ORDER.iter().enumerate() {
            match self.records.get(index) {
                Some(record) if record.step == *expected => {}
                found => {
                    return Err(StepOrderError {
                        index,
                        expected: *expected,
                        found: found.map(|record| record.step),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The Phase A steps did not run in their canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepOrderError {
    /// Position at which the order first diverged.
    pub index: usize,
    /// Step the canonical order requires at this position.
    pub expected: BootstrapStep,
    /// Step actually recorded at this position, if any.
    pub found: Option<BootstrapStep>,
}

impl fmt::Display for StepOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.found {
            Some(found) => write!(
                f,
                "bootstrap step order violated at position {}: expected {}, found {}",
                self.index, self.expected, found
            ),
            None => write!(
                f,
                "bootstrap step order violated at position {}: expected {}, found end of log",
                self.index, self.expected
            ),
        }
    }
}

impl std::error::Error for StepOrderError {}

#[cfg(test)]
mod tests;
