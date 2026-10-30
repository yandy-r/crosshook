//! Windows-version restore journal policy (D7; invariants I1–I14 of
//! `research-journal.md`).
//!
//! The journal row is written `pending` before a dependency install mutates
//! the prefix and is deleted only after a restore verified against
//! `system.reg`. Every other outcome leaves the row (`failed` when possible),
//! so the prefix stays blocked until a verified restore. All calls use the
//! canonical `pfx` path; callers that restore hold the prefix job key.
//! Startup recovery moves a row stored under a path that now resolves to
//! another `pfx` (a prefix moved behind a symlink) to that canonical key.

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use crosshook_core::metadata::PrefixVersionRestoreJournalRow;
use crosshook_core::prefix_deps::runner::{normalize_prefix_path, sanitize_output_for_ui};
use crosshook_core::prefix_deps::{
    PrefixVersionRestoreOutcome, PrefixVersionRestorePlan, PrefixVersionRestoreState,
};
use tokio_util::sync::CancellationToken;

use super::messages::{
    msg_cleanup_failed, MSG_REPAIR_REQUIRED, MSG_RESTORE_FAILED, MSG_RESTORE_INVALID_STATE,
};
use super::seams::{JournalStore, PrefixInUse, PrefixVersionRestorer};
use super::types::PrefixVersionRepairStatus;
use super::RecoveryLedger;
use crate::services::BoxFuture;
use crate::{AppError, AppErrorKind};

mod entry;

use entry::{parse_tool, tool_name, validate_prefix_path, STATE_FAILED, STATE_PENDING};
pub(crate) use entry::{JournalEntry, PendingOrFailed};

/// What a restore outcome does to the journal row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JournalDecision {
    Delete,
    MarkFailed(String),
    NoMutation,
}

/// Only a verified restore deletes the row (I2); a failure records its error.
pub(crate) fn decide(state: PrefixVersionRestoreState, error: Option<&str>) -> JournalDecision {
    match state {
        PrefixVersionRestoreState::Succeeded => JournalDecision::Delete,
        PrefixVersionRestoreState::Failed => {
            JournalDecision::MarkFailed(error.unwrap_or(MSG_RESTORE_FAILED).to_string())
        }
        PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Pending => {
            JournalDecision::NoMutation
        }
    }
}

/// Proof that [`PrefixVersionJournal::record`] persisted the `pending` row (I1).
#[must_use = "resolve the ticket after the install child exits"]
#[derive(Debug)]
pub(crate) struct JournalTicket {
    prefix: String,
}

/// Result of one recovery attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryResult {
    /// No row (or it vanished under the key).
    Clean,
    /// Verified restore; row deleted.
    Restored,
    /// Restore failed; row marked `failed` when possible.
    RestoreFailed { error: String },
    /// Verified restore, but the row could not be deleted; row `failed` when possible.
    CleanupFailed { error: String },
    /// Row failed validation; marked `failed`, never executed.
    Corrupt { error: String },
    /// `failed` row while failed rows are excluded (startup, pre-install).
    SkippedFailed,
    /// A live process uses the prefix; row left `pending`.
    SkippedInUse,
    /// Another job holds the prefix key.
    SkippedBusy,
    /// The job was cancelled (app shutdown) before the restore started, or
    /// the runtime shut down; the row was left as it was.
    SkippedCancelled,
    /// The journal store is unavailable.
    StoreUnavailable,
}

/// A journal row for startup recovery: the key it is stored under and the
/// canonical `pfx` that path resolves to now (the stored path itself when it
/// no longer resolves). They differ for a prefix moved behind a symlink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryRow {
    pub(crate) stored: String,
    pub(crate) canonical: String,
}

/// Install gate after pre-install recovery: only a clean or restored prefix
/// may be mutated.
///
/// # Errors
/// `Unavailable` when the journal store is unavailable; otherwise
/// `Conflict` [`MSG_REPAIR_REQUIRED`].
pub(crate) fn install_gate(result: &RecoveryResult) -> Result<(), AppError> {
    match result {
        RecoveryResult::Clean | RecoveryResult::Restored => Ok(()),
        RecoveryResult::StoreUnavailable => Err(AppError::new(
            AppErrorKind::Unavailable,
            "metadata database is unavailable",
        )),
        RecoveryResult::SkippedCancelled => Err(AppError::new(
            AppErrorKind::Cancelled,
            "the prefix job was cancelled",
        )),
        RecoveryResult::RestoreFailed { .. }
        | RecoveryResult::CleanupFailed { .. }
        | RecoveryResult::Corrupt { .. }
        | RecoveryResult::SkippedFailed
        | RecoveryResult::SkippedInUse
        | RecoveryResult::SkippedBusy => {
            Err(AppError::new(AppErrorKind::Conflict, MSG_REPAIR_REQUIRED))
        }
    }
}

/// Journal policy over the injectable store, restorer and in-use probe.
#[derive(Clone)]
pub(crate) struct PrefixVersionJournal {
    store: Arc<dyn JournalStore>,
    restorer: Arc<dyn PrefixVersionRestorer>,
    in_use: Arc<dyn PrefixInUse>,
}

impl PrefixVersionJournal {
    pub(crate) fn new(
        store: Arc<dyn JournalStore>,
        restorer: Arc<dyn PrefixVersionRestorer>,
        in_use: Arc<dyn PrefixInUse>,
    ) -> Self {
        Self {
            store,
            restorer,
            in_use,
        }
    }

    /// Whether the journal store can be read and written.
    pub(crate) fn is_available(&self) -> bool {
        self.store.is_available()
    }

    /// Runs the restorer on `plan` (not bound to any job token, I8).
    pub(crate) fn restore(
        &self,
        plan: PrefixVersionRestorePlan,
    ) -> BoxFuture<'static, PrefixVersionRestoreOutcome> {
        self.restorer.restore(plan)
    }

    /// Rows startup recovery handles: every `pending` row, and every `failed`
    /// row stored under a key that now resolves to another canonical `pfx`
    /// (so recovery re-keys it where status and repair look). Blocking
    /// (canonicalizes each stored path).
    ///
    /// # Errors
    /// The store's listing error.
    pub(crate) fn recovery_rows(&self) -> Result<Vec<RecoveryRow>, AppError> {
        Ok(self
            .store
            .list()?
            .into_iter()
            .filter_map(|row| {
                let canonical = normalize_prefix_path(&row.prefix_path)
                    .unwrap_or_else(|_| row.prefix_path.clone());
                (row.state == STATE_PENDING || canonical != row.prefix_path).then_some(
                    RecoveryRow {
                        stored: row.prefix_path,
                        canonical,
                    },
                )
            })
            .collect())
    }

    /// Persisted status of `canonical`; `in_progress`/`last_recovery` are
    /// filled by the service.
    ///
    /// # Errors
    /// The store's load error, or `Validation { field: "prefix_path" }` for a
    /// row with an unknown tool or state (fail closed, I7).
    pub(crate) fn status(&self, canonical: &str) -> Result<PrefixVersionRepairStatus, AppError> {
        let Some(row) = self.store.load(canonical)? else {
            return Ok(PrefixVersionRepairStatus::clean());
        };
        let validated = parse_tool(&row.tool_type).and_then(|_| PendingOrFailed::parse(&row.state));
        let state = validated.map_err(|error| AppError::validation("prefix_path", error))?;
        Ok(PrefixVersionRepairStatus {
            required: true,
            state: Some(state.restore_state()),
            last_error: row.last_error,
            in_progress: false,
            last_recovery: None,
        })
    }

    /// Persists `plan` as a `pending` row before the install spawns (I1).
    ///
    /// # Errors
    /// The store's upsert error; the caller must not spawn then.
    pub(crate) fn record(
        &self,
        plan: &PrefixVersionRestorePlan,
        profile_id: &str,
    ) -> Result<JournalTicket, AppError> {
        let now = Utc::now().to_rfc3339();
        self.store.upsert_pending(&PrefixVersionRestoreJournalRow {
            prefix_path: plan.prefix_path.clone(),
            profile_id: Some(profile_id.to_string()),
            binary_path: plan.binary_path.clone(),
            tool_type: tool_name(plan.tool_type).to_string(),
            steam_app_id: plan.steam_app_id.clone(),
            restore_verb: plan.restore_verb.clone(),
            state: STATE_PENDING.to_string(),
            last_error: None,
            created_at: now.clone(),
            updated_at: now,
        })?;
        Ok(JournalTicket {
            prefix: plan.prefix_path.clone(),
        })
    }

    /// Applies the install's restore outcome to the row and returns the
    /// outcome to report: a verified restore whose row cannot be deleted is
    /// reported `Failed` and the row marked `failed` when possible (I3).
    pub(crate) fn resolve(
        &self,
        ticket: JournalTicket,
        outcome: PrefixVersionRestoreOutcome,
    ) -> PrefixVersionRestoreOutcome {
        let prefix = ticket.prefix;
        match decide(outcome.state, outcome.error.as_deref()) {
            JournalDecision::Delete => match self.store.delete(&prefix) {
                Ok(()) => outcome,
                Err(error) => {
                    let message = msg_cleanup_failed(&sanitize_output_for_ui(&error.to_string()));
                    self.mark_failed(&prefix, &message);
                    PrefixVersionRestoreOutcome {
                        state: PrefixVersionRestoreState::Failed,
                        error: Some(message),
                    }
                }
            },
            JournalDecision::MarkFailed(message) => {
                self.mark_failed(&prefix, &message);
                outcome
            }
            JournalDecision::NoMutation => outcome,
        }
    }

    /// Restores, verifies and resolves one prefix. The CALLER HOLDS the prefix
    /// key. `include_failed=false` skips `failed` rows; an invalid row is
    /// marked `failed` and never executed; with `check_in_use`, a prefix used
    /// by a live process is skipped and its row stays `pending`. A `cancel`led
    /// job never starts the restore. Store, validation and in-use work runs
    /// on the blocking pool; only the restore runs on the executor.
    pub(crate) async fn recover(
        &self,
        canonical: &str,
        include_failed: bool,
        check_in_use: bool,
        cancel: &CancellationToken,
    ) -> RecoveryResult {
        self.recover_keyed(canonical, None, include_failed, check_in_use, cancel)
            .await
    }

    /// Startup recovery of `row` under the key of `row.canonical`: a row
    /// stored under another path (a moved prefix) is first moved to the
    /// canonical key, then restored like [`Self::recover`] with failed rows
    /// skipped and the in-use check on.
    pub(crate) async fn recover_row(
        &self,
        row: &RecoveryRow,
        cancel: &CancellationToken,
    ) -> RecoveryResult {
        self.recover_keyed(&row.canonical, Some(&row.stored), false, true, cancel)
            .await
    }

    async fn recover_keyed(
        &self,
        canonical: &str,
        stored: Option<&str>,
        include_failed: bool,
        check_in_use: bool,
        cancel: &CancellationToken,
    ) -> RecoveryResult {
        let journal = self.clone();
        let key = canonical.to_string();
        let stored = stored.map(str::to_string);
        let prepared = blocking(move || {
            journal.prepare(&key, stored.as_deref(), include_failed, check_in_use)
        })
        .await;
        let plan = match prepared {
            Some(Ok(plan)) if !cancel.is_cancelled() => plan,
            Some(Ok(_)) | None => return RecoveryResult::SkippedCancelled,
            Some(Err(result)) => return result,
        };
        let outcome = self.restorer.restore(plan).await;
        let journal = self.clone();
        let key = canonical.to_string();
        blocking(move || journal.finish(&key, outcome))
            .await
            .unwrap_or(RecoveryResult::SkippedCancelled)
    }

    /// Blocking part of a recovery up to the restore: re-key, load, validate
    /// and the in-use check. `Err` is the final result.
    fn prepare(
        &self,
        canonical: &str,
        stored: Option<&str>,
        include_failed: bool,
        check_in_use: bool,
    ) -> Result<PrefixVersionRestorePlan, RecoveryResult> {
        if !self.store.is_available() {
            return Err(RecoveryResult::StoreUnavailable);
        }
        if let Some(stored) = stored.filter(|stored| *stored != canonical) {
            if let Err(error) = self.store.rekey(stored, canonical) {
                tracing::warn!(%error, "failed to move prefix repair journal row");
                return Err(RecoveryResult::StoreUnavailable);
            }
        }
        let row = match self.store.load(canonical) {
            Ok(Some(row)) => row,
            Ok(None) => return Err(RecoveryResult::Clean),
            Err(error) => {
                tracing::warn!(%error, "failed to load prefix repair journal");
                return Err(RecoveryResult::StoreUnavailable);
            }
        };
        if row.state == STATE_FAILED && !include_failed {
            return Err(RecoveryResult::SkippedFailed);
        }
        let entry = validate_prefix_path(canonical, &row.prefix_path)
            .and_then(|()| JournalEntry::try_from(row))
            .map_err(|error| {
                let error = sanitize_output_for_ui(&error);
                self.mark_failed(canonical, &error);
                RecoveryResult::Corrupt { error }
            })?;
        if check_in_use && self.in_use.in_use(Path::new(canonical)) {
            return Err(RecoveryResult::SkippedInUse);
        }
        Ok(entry.plan)
    }

    /// Blocking resolution of the restore `outcome`.
    fn finish(&self, canonical: &str, outcome: PrefixVersionRestoreOutcome) -> RecoveryResult {
        let error = match outcome.state {
            PrefixVersionRestoreState::Succeeded => return self.finish_verified(canonical),
            PrefixVersionRestoreState::Failed => outcome
                .error
                .unwrap_or_else(|| MSG_RESTORE_FAILED.to_string()),
            PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Pending => {
                MSG_RESTORE_INVALID_STATE.to_string()
            }
        };
        self.mark_failed(canonical, &error);
        RecoveryResult::RestoreFailed { error }
    }

    /// Pre-install recovery under the install job's key: pending rows are
    /// restored, `failed` rows are not retried. The result is recorded in
    /// `ledger` before the gate, so a failure replaces a stale success.
    ///
    /// # Errors
    /// See [`install_gate`].
    pub(crate) async fn recover_before_install(
        &self,
        canonical: &str,
        ledger: &RecoveryLedger,
        cancel: &CancellationToken,
    ) -> Result<RecoveryResult, AppError> {
        let result = self.recover(canonical, false, false, cancel).await;
        ledger.record(canonical, &result);
        install_gate(&result)?;
        Ok(result)
    }

    /// Deletes the row after a verified restore; a row that vanished under the
    /// key is clean (I4).
    fn finish_verified(&self, canonical: &str) -> RecoveryResult {
        match self.store.load(canonical) {
            Ok(None) => return RecoveryResult::Clean,
            Ok(Some(_)) => {}
            Err(error) => tracing::warn!(%error, "failed to reload prefix repair journal"),
        }
        match self.store.delete(canonical) {
            Ok(()) => RecoveryResult::Restored,
            Err(error) => {
                let error = msg_cleanup_failed(&sanitize_output_for_ui(&error.to_string()));
                self.mark_failed(canonical, &error);
                RecoveryResult::CleanupFailed { error }
            }
        }
    }

    /// Best-effort `failed` mark; on error the row stays as it is.
    fn mark_failed(&self, canonical: &str, error: &str) {
        if let Err(persist_error) = self.store.mark_failed(canonical, error) {
            tracing::warn!(%persist_error, "failed to mark prefix repair journal failed");
        }
    }
}

/// Runs `work` on the blocking pool. A panic resumes on the caller (so it
/// fails the job as before); `None` when the runtime is shutting down.
async fn blocking<T, F>(work: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => Some(value),
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(_) => None,
    }
}
