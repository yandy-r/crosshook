//! Windows-version restore journal policy (D7; invariants I1–I14 of
//! `research-journal.md`).
//!
//! The journal row is written `pending` before a dependency install mutates
//! the prefix and is deleted only after a restore verified against
//! `system.reg`. Every other outcome leaves the row (`failed` when possible),
//! so the prefix stays blocked until a verified restore. All calls use the
//! canonical `pfx` path; callers that restore hold the prefix job key.

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use crosshook_core::metadata::PrefixVersionRestoreJournalRow;
use crosshook_core::platform::normalized_path_is_executable_file_on_host;
use crosshook_core::prefix_deps::runner::{is_windows_version_verb, sanitize_output_for_ui};
use crosshook_core::prefix_deps::validation::{
    parse_nonzero_steam_app_id, validate_protontricks_verbs,
};
use crosshook_core::prefix_deps::{
    PrefixDepsTool, PrefixVersionRestoreOutcome, PrefixVersionRestorePlan,
    PrefixVersionRestoreState,
};

use super::messages::{
    msg_cleanup_failed, MSG_REPAIR_REQUIRED, MSG_RESTORE_FAILED, MSG_RESTORE_INVALID_STATE,
};
use super::seams::{JournalStore, PrefixInUse, PrefixVersionRestorer};
use super::types::PrefixVersionRepairStatus;
use super::RecoveryLedger;
use crate::services::BoxFuture;
use crate::{AppError, AppErrorKind};

const STATE_PENDING: &str = "pending";
const STATE_FAILED: &str = "failed";

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

/// Persisted row state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingOrFailed {
    Pending,
    Failed,
}

impl PendingOrFailed {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            STATE_PENDING => Ok(Self::Pending),
            STATE_FAILED => Ok(Self::Failed),
            other => Err(format!("invalid repair state in journal: {other}")),
        }
    }

    fn restore_state(self) -> PrefixVersionRestoreState {
        match self {
            Self::Pending => PrefixVersionRestoreState::Pending,
            Self::Failed => PrefixVersionRestoreState::Failed,
        }
    }
}

/// A journal row validated for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JournalEntry {
    pub(crate) plan: PrefixVersionRestorePlan,
    pub(crate) state: PendingOrFailed,
    pub(crate) last_error: Option<String>,
}

fn parse_tool(raw: &str) -> Result<PrefixDepsTool, String> {
    match raw {
        "winetricks" => Ok(PrefixDepsTool::Winetricks),
        "protontricks" => Ok(PrefixDepsTool::Protontricks),
        other => Err(format!(
            "invalid dependency tool in repair journal: {other}"
        )),
    }
}

fn tool_name(tool: PrefixDepsTool) -> &'static str {
    match tool {
        PrefixDepsTool::Winetricks => "winetricks",
        PrefixDepsTool::Protontricks => "protontricks",
    }
}

/// M4: a replayed row runs only a bare tool name or an absolute executable.
fn validate_binary_path(binary_path: &str) -> Result<(), String> {
    if matches!(binary_path, "winetricks" | "protontricks") {
        return Ok(());
    }
    if Path::new(binary_path).is_absolute()
        && normalized_path_is_executable_file_on_host(binary_path)
    {
        return Ok(());
    }
    Err("repair journal has an invalid tool binary path".to_string())
}

impl TryFrom<PrefixVersionRestoreJournalRow> for JournalEntry {
    type Error = String;

    /// Validates tool, state, restore verb (I9), the protontricks App ID and
    /// the binary path (M4); the error text never embeds the binary path.
    fn try_from(row: PrefixVersionRestoreJournalRow) -> Result<Self, Self::Error> {
        let tool_type = parse_tool(&row.tool_type)?;
        let state = PendingOrFailed::parse(&row.state)?;
        validate_protontricks_verbs(std::slice::from_ref(&row.restore_verb))
            .map_err(|error| error.to_string())?;
        if !is_windows_version_verb(&row.restore_verb) {
            return Err("repair journal has an invalid Windows version restore verb".to_string());
        }
        let steam_app_id = match tool_type {
            PrefixDepsTool::Protontricks => Some(
                row.steam_app_id
                    .as_deref()
                    .and_then(parse_nonzero_steam_app_id)
                    .ok_or_else(|| {
                        "repair journal has an invalid Steam App ID for protontricks; expected a positive decimal value"
                            .to_string()
                    })?,
            ),
            PrefixDepsTool::Winetricks => None,
        };
        validate_binary_path(&row.binary_path)?;
        Ok(Self {
            plan: PrefixVersionRestorePlan {
                binary_path: row.binary_path,
                prefix_path: row.prefix_path,
                tool_type,
                steam_app_id,
                restore_verb: row.restore_verb,
            },
            state,
            last_error: row.last_error,
        })
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
    /// The journal store is unavailable.
    StoreUnavailable,
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

    /// Canonical prefixes of every `pending` row.
    ///
    /// # Errors
    /// The store's listing error.
    pub(crate) fn pending_prefixes(&self) -> Result<Vec<String>, AppError> {
        Ok(self
            .store
            .list()?
            .into_iter()
            .filter(|row| row.state == STATE_PENDING)
            .map(|row| row.prefix_path)
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
    /// by a live process is skipped and its row stays `pending`.
    pub(crate) async fn recover(
        &self,
        canonical: &str,
        include_failed: bool,
        check_in_use: bool,
    ) -> RecoveryResult {
        if !self.store.is_available() {
            return RecoveryResult::StoreUnavailable;
        }
        let row = match self.store.load(canonical) {
            Ok(Some(row)) => row,
            Ok(None) => return RecoveryResult::Clean,
            Err(error) => {
                tracing::warn!(%error, "failed to load prefix repair journal");
                return RecoveryResult::StoreUnavailable;
            }
        };
        if row.state == STATE_FAILED && !include_failed {
            return RecoveryResult::SkippedFailed;
        }
        let entry = match JournalEntry::try_from(row) {
            Ok(entry) => entry,
            Err(error) => {
                let error = sanitize_output_for_ui(&error);
                self.mark_failed(canonical, &error);
                return RecoveryResult::Corrupt { error };
            }
        };
        if check_in_use && self.in_use.in_use(Path::new(canonical)) {
            return RecoveryResult::SkippedInUse;
        }
        let outcome = self.restorer.restore(entry.plan).await;
        match outcome.state {
            PrefixVersionRestoreState::Succeeded => self.finish_verified(canonical),
            PrefixVersionRestoreState::Failed => {
                let error = outcome
                    .error
                    .unwrap_or_else(|| MSG_RESTORE_FAILED.to_string());
                self.mark_failed(canonical, &error);
                RecoveryResult::RestoreFailed { error }
            }
            PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Pending => {
                let error = MSG_RESTORE_INVALID_STATE.to_string();
                self.mark_failed(canonical, &error);
                RecoveryResult::RestoreFailed { error }
            }
        }
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
    ) -> Result<RecoveryResult, AppError> {
        let result = self.recover(canonical, false, false).await;
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
