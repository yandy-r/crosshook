//! Prefix dependency (winetricks/protontricks) checks and installs, run as
//! supervised jobs keyed on the canonical prefix (D5), with the restart-safe
//! Windows-version restore journal (D7) and its startup recovery (D8).

mod check;
mod install;
mod journal;
mod messages;
mod recovery;
mod repair;
mod seams;
mod status;
mod tools;
mod types;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;

use crate::jobs::{JobRegistry, LOG_DIR};

pub use messages::{
    msg_already_installing, MSG_APP_ID_REQUIRED, MSG_NO_TOOL_CHECK, MSG_NO_TOOL_INSTALL,
    MSG_OFFLINE, MSG_REPAIR_REQUIRED,
};
pub use tools::{profile_steam_app_id, resolve_tool, ResolvedTool, ToolSelectionError};
pub use types::{
    completion_payload, DepCompletePayload, DepInstallCompletion, DepLogPayload, LastRecovery,
    PrefixVersionRepairStatus, RecoveryOutcome,
};

use journal::{PrefixVersionJournal, RecoveryResult};
use seams::{CoreRestorer, CoreToolProbe, MetadataJournalStore, ProcPrefixInUse, ToolProbe};
#[cfg(test)]
use seams::{JournalStore, PrefixInUse, PrefixVersionRestorer};

/// Registry kind of a dependency install job.
pub const PREFIX_DEPS_INSTALL_JOB_KIND: &str = "prefix-deps-install";
/// Registry kind of an explicit Windows-version repair job.
pub const PREFIX_REPAIR_JOB_KIND: &str = "prefix-version-repair";
/// Registry kind of an automatic (startup) recovery job.
pub const PREFIX_RECOVERY_JOB_KIND: &str = "prefix-version-recovery";

/// Last automatic recovery per canonical prefix, for this session only.
#[derive(Clone, Default)]
pub(crate) struct RecoveryLedger(Arc<Mutex<HashMap<String, LastRecovery>>>);

impl RecoveryLedger {
    /// Records `result` for `canonical`: `Restored` → restored; a failed
    /// restore, cleanup or corrupt row → failed with its error. Other results
    /// leave the entry unchanged.
    pub(crate) fn record(&self, canonical: &str, result: &RecoveryResult) {
        let entry = match result {
            RecoveryResult::Restored => LastRecovery {
                outcome: RecoveryOutcome::Restored,
                message: None,
            },
            RecoveryResult::RestoreFailed { error }
            | RecoveryResult::CleanupFailed { error }
            | RecoveryResult::Corrupt { error } => LastRecovery {
                outcome: RecoveryOutcome::Failed,
                message: Some(error.clone()),
            },
            RecoveryResult::Clean
            | RecoveryResult::SkippedFailed
            | RecoveryResult::SkippedInUse
            | RecoveryResult::SkippedBusy
            | RecoveryResult::StoreUnavailable => return,
        };
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(canonical.to_string(), entry);
    }

    /// Forgets the recorded recovery of `canonical` (an explicit repair
    /// superseded it; repairs are never reported as automatic recoveries).
    pub(crate) fn clear(&self, canonical: &str) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(canonical);
    }

    /// The last recorded recovery of `canonical`.
    pub(crate) fn get(&self, canonical: &str) -> Option<LastRecovery> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(canonical)
            .cloned()
    }
}

/// Toolkit-independent prefix dependency operations.
#[derive(Clone)]
pub struct PrefixDepsService {
    settings: SettingsStore,
    profiles: ProfileStore,
    metadata: MetadataStore,
    jobs: JobRegistry,
    journal: PrefixVersionJournal,
    tools: Arc<dyn ToolProbe>,
    recoveries: RecoveryLedger,
    log_dir: PathBuf,
}

impl PrefixDepsService {
    /// Builds the service over the shared stores and job registry with the
    /// core tool detection, metadata journal, restorer and `/proc` probe.
    pub fn new(
        settings: SettingsStore,
        profiles: ProfileStore,
        metadata: MetadataStore,
        jobs: JobRegistry,
    ) -> Self {
        let journal = PrefixVersionJournal::new(
            Arc::new(MetadataJournalStore(metadata.clone())),
            Arc::new(CoreRestorer),
            Arc::new(ProcPrefixInUse),
        );
        Self {
            settings,
            profiles,
            metadata,
            jobs,
            journal,
            tools: Arc::new(CoreToolProbe),
            recoveries: RecoveryLedger::default(),
            log_dir: PathBuf::from(LOG_DIR),
        }
    }

    /// Builds the service with injected seams and log directory.
    #[cfg(test)]
    pub(crate) fn with_seams(
        settings: SettingsStore,
        profiles: ProfileStore,
        metadata: MetadataStore,
        jobs: JobRegistry,
        store: Arc<dyn JournalStore>,
        restorer: Arc<dyn PrefixVersionRestorer>,
        in_use: Arc<dyn PrefixInUse>,
        tools: Arc<dyn ToolProbe>,
        log_dir: PathBuf,
    ) -> Self {
        Self {
            settings,
            profiles,
            metadata,
            jobs,
            journal: PrefixVersionJournal::new(store, restorer, in_use),
            tools,
            recoveries: RecoveryLedger::default(),
            log_dir,
        }
    }

    /// The job registry the service's jobs run on.
    #[cfg(test)]
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }
}

static_assertions::assert_impl_all!(PrefixDepsService: Send, Sync);

#[cfg(test)]
mod tests;
