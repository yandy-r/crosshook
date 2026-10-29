//! Restore-journal resolution and decision tests (feature-spec AC 815-1:
//! J4a–J4c, J4f–J4h) plus the ported Tauri-command tests (row status
//! mapping, persisted App ID, completion payload).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crosshook_core::metadata::{MetadataStore, MetadataStoreError, PrefixVersionRestoreJournalRow};
use crosshook_core::prefix_deps::runner::prepare_install_packages;
use crosshook_core::prefix_deps::{
    PrefixDependencyInstallOutcome, PrefixDepsTool, PrefixVersionRestoreOutcome,
    PrefixVersionRestorePlan, PrefixVersionRestoreState,
};

use super::super::journal::{
    decide, install_gate, JournalDecision, JournalEntry, PendingOrFailed, PrefixVersionJournal,
    RecoveryResult,
};
use super::super::seams::{JournalStore, MetadataJournalStore};
use super::super::{completion_payload, RecoveryLedger, RecoveryOutcome, MSG_REPAIR_REQUIRED};
use super::fakes::FakeInUse;
use super::helpers::{pending_row, Harness, ToolScript};
use crate::AppErrorKind;

// J4a: only a verified restore deletes (ported journal_decision test).
#[test]
fn j4a_decide_only_deletes_verified_success() {
    assert_eq!(
        decide(PrefixVersionRestoreState::Succeeded, None),
        JournalDecision::Delete
    );
    assert_eq!(
        decide(PrefixVersionRestoreState::Failed, Some("restore timed out")),
        JournalDecision::MarkFailed("restore timed out".to_string())
    );
    assert_eq!(
        decide(PrefixVersionRestoreState::Failed, None),
        JournalDecision::MarkFailed("Windows version restoration failed".to_string())
    );
    for state in [
        PrefixVersionRestoreState::NotRequired,
        PrefixVersionRestoreState::Pending,
    ] {
        assert_eq!(decide(state, Some("ignored")), JournalDecision::NoMutation);
    }
}

fn plan_for(h: &Harness, binary: &str) -> PrefixVersionRestorePlan {
    JournalEntry::try_from(pending_row(&h.prefix, binary))
        .unwrap()
        .plan
}

fn succeeded() -> PrefixVersionRestoreOutcome {
    PrefixVersionRestoreOutcome {
        state: PrefixVersionRestoreState::Succeeded,
        error: None,
    }
}

// J4b: a delete failure after a verified restore marks the row failed and
// reports Failed (ported journal_delete_failure_keeps_completion_unsafe).
#[test]
fn j4b_delete_failure_after_verified_restore_marks_failed() {
    let h = Harness::new();
    let store = h.recording_store();
    let journal = PrefixVersionJournal::new(
        store.clone(),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );
    let ticket = journal
        .record(&plan_for(&h, "winetricks"), &h.profile_id)
        .unwrap();
    assert_eq!(
        h.row().unwrap().profile_id.as_deref(),
        Some(h.profile_id.as_str())
    );
    store.fail_delete.store(true, Ordering::SeqCst);

    let outcome = journal.resolve(ticket, succeeded());

    assert_eq!(outcome.state, PrefixVersionRestoreState::Failed);
    let error = outcome.error.clone().unwrap();
    assert!(error.contains("cleanup failed"), "{error}");
    let ops = store.ops();
    assert_eq!(ops[..2], ["upsert".to_string(), "delete".to_string()]);
    assert!(ops[2].starts_with("mark_failed:") && ops[2].contains("cleanup failed"));
    let row = h.row().unwrap();
    assert_eq!(row.state, "failed");
    assert_eq!(row.last_error.as_deref(), Some(error.as_str()));

    let payload = completion_payload("profile", "/raw/prefix", install_outcome(outcome));
    assert!(!payload.succeeded);
    assert!(payload.install_succeeded);
}

// J4c: when both delete and mark fail the outcome is still Failed and the
// row stays pending.
#[test]
fn j4c_delete_and_mark_failure_leaves_row_pending() {
    let h = Harness::new();
    let store = h.recording_store();
    let journal = PrefixVersionJournal::new(
        store.clone(),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );
    let ticket = journal
        .record(&plan_for(&h, "winetricks"), &h.profile_id)
        .unwrap();
    store.fail_delete.store(true, Ordering::SeqCst);
    store.fail_mark.store(true, Ordering::SeqCst);

    let outcome = journal.resolve(ticket, succeeded());

    assert_eq!(outcome.state, PrefixVersionRestoreState::Failed);
    assert_eq!(h.row().unwrap().state, "pending");
}

// J4c': a failed install restore marks the row and keeps the reported error.
#[test]
fn j4c_failed_restore_resolution_marks_row_failed() {
    let h = Harness::new();
    let (journal, _) = h.journal(false);
    let ticket = journal
        .record(&plan_for(&h, "winetricks"), &h.profile_id)
        .unwrap();
    let failed = PrefixVersionRestoreOutcome {
        state: PrefixVersionRestoreState::Failed,
        error: Some("restore timed out".to_string()),
    };

    assert_eq!(journal.resolve(ticket, failed.clone()), failed);
    let row = h.row().unwrap();
    assert_eq!(row.state, "failed");
    assert_eq!(row.last_error.as_deref(), Some("restore timed out"));
}

#[test]
fn journal_entry_accepts_bare_tool_names_and_canonicalizes_app_id() {
    let mut row = pending_row("/games/example/pfx", "protontricks");
    row.tool_type = "protontricks".to_string();
    row.steam_app_id = Some(" 12345 ".to_string());
    let entry = JournalEntry::try_from(row).unwrap();
    assert_eq!(entry.plan.steam_app_id.as_deref(), Some("12345"));
    assert_eq!(entry.plan.tool_type, PrefixDepsTool::Protontricks);
    assert_eq!(entry.state, PendingOrFailed::Pending);

    let mut failed = pending_row("/games/example/pfx", "winetricks");
    failed.state = "failed".to_string();
    failed.last_error = Some("restore timed out".to_string());
    failed.steam_app_id = Some("570".to_string());
    let entry = JournalEntry::try_from(failed).unwrap();
    assert_eq!(entry.state, PendingOrFailed::Failed);
    assert_eq!(entry.last_error.as_deref(), Some("restore timed out"));
    assert_eq!(entry.plan.steam_app_id, None, "winetricks drops a stray id");
}

#[test]
fn journal_entry_rejects_invalid_state_and_tool() {
    let mut state = pending_row("/games/example/pfx", "winetricks");
    state.state = "succeeded".to_string();
    assert!(JournalEntry::try_from(state)
        .unwrap_err()
        .contains("invalid repair state"));

    let mut tool = pending_row("/games/example/pfx", "winetricks");
    tool.tool_type = "wine".to_string();
    assert!(JournalEntry::try_from(tool)
        .unwrap_err()
        .contains("invalid dependency tool"));
}

/// Store serving one fixed raw row (for rows the table's CHECKs reject).
struct RawRowStore(PrefixVersionRestoreJournalRow);

impl JournalStore for RawRowStore {
    fn is_available(&self) -> bool {
        true
    }
    fn load(
        &self,
        prefix: &str,
    ) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
        assert_eq!(prefix, self.0.prefix_path);
        Ok(Some(self.0.clone()))
    }
    fn list(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
        Ok(vec![self.0.clone()])
    }
    fn upsert_pending(
        &self,
        _row: &PrefixVersionRestoreJournalRow,
    ) -> Result<(), MetadataStoreError> {
        panic!("status must not write")
    }
    fn mark_failed(&self, _prefix: &str, _error: &str) -> Result<(), MetadataStoreError> {
        panic!("status must not write")
    }
    fn delete(&self, _prefix: &str) -> Result<(), MetadataStoreError> {
        panic!("status must not write")
    }
}

// Ported row_status_mapping_* tests.
#[test]
fn status_maps_pending_and_failed_rows_and_rejects_invalid_ones() {
    let h = Harness::new();
    let (journal, _) = h.journal(false);
    assert!(!journal.status(&h.prefix).unwrap().required);

    h.seed_row(&pending_row(&h.prefix, "winetricks"));
    let pending = journal.status(&h.prefix).unwrap();
    assert!(pending.required && !pending.in_progress);
    assert_eq!(pending.state, Some(PrefixVersionRestoreState::Pending));
    assert_eq!(pending.last_error, None);

    h.fail_row("restore timed out");
    let failed = journal.status(&h.prefix).unwrap();
    assert_eq!(failed.state, Some(PrefixVersionRestoreState::Failed));
    assert_eq!(failed.last_error.as_deref(), Some("restore timed out"));

    for (state, tool) in [("succeeded", "winetricks"), ("pending", "wine")] {
        let mut row = pending_row("/games/example/pfx", "winetricks");
        row.state = state.to_string();
        row.tool_type = tool.to_string();
        let journal = PrefixVersionJournal::new(
            Arc::new(RawRowStore(row)),
            h.restorer(),
            Arc::new(FakeInUse::new(false, "/games/example/pfx")),
        );
        let error = journal.status("/games/example/pfx").unwrap_err();
        assert!(matches!(
            error.kind(),
            AppErrorKind::Validation {
                field: "prefix_path"
            }
        ));
    }
}

// J4f (journal level): a failed record returns Err and persists nothing.
#[test]
fn j4f_failed_record_returns_error_and_writes_nothing() {
    let h = Harness::new();
    let store = h.recording_store();
    store.fail_upsert.store(true, Ordering::SeqCst);
    let journal = PrefixVersionJournal::new(
        store,
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );

    assert!(journal
        .record(&plan_for(&h, "winetricks"), &h.profile_id)
        .is_err());
    assert!(h.row().is_none());

    let disabled = PrefixVersionJournal::new(
        Arc::new(MetadataJournalStore(MetadataStore::disabled())),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );
    let error = disabled
        .record(&plan_for(&h, "winetricks"), &h.profile_id)
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
}

// J4g (journal level): a recorded row stays pending until resolved, so a
// spawn failure (ticket never resolved) leaves the prefix blocked.
#[test]
fn j4g_unresolved_ticket_keeps_row_pending() {
    let h = Harness::new();
    let (journal, _) = h.journal(false);
    let ticket = journal
        .record(&plan_for(&h, "winetricks"), &h.profile_id)
        .unwrap();
    drop(ticket);
    assert_eq!(h.row().unwrap().state, "pending");
    assert!(journal.status(&h.prefix).unwrap().required);
    assert_eq!(journal.pending_prefixes().unwrap(), vec![h.prefix.clone()]);
}

// J4h: an explicit Windows-version verb produces no plan, so nothing is recorded.
#[test]
fn j4h_explicit_version_verb_has_no_plan() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let prepared = prepare_install_packages(
        &tool,
        &h.prefix,
        &["win7".to_string()],
        PrefixDepsTool::Winetricks,
        None,
    )
    .unwrap();
    assert!(prepared.restore_plan().is_none());
    assert!(h.row().is_none());
}

#[test]
fn install_gate_allows_only_clean_or_restored() {
    assert!(install_gate(&RecoveryResult::Clean).is_ok());
    assert!(install_gate(&RecoveryResult::Restored).is_ok());
    for blocked in [
        RecoveryResult::RestoreFailed { error: "e".into() },
        RecoveryResult::CleanupFailed { error: "e".into() },
        RecoveryResult::Corrupt { error: "e".into() },
        RecoveryResult::SkippedFailed,
        RecoveryResult::SkippedInUse,
        RecoveryResult::SkippedBusy,
    ] {
        let error = install_gate(&blocked).unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Conflict, "{blocked:?}");
        assert_eq!(error.message(), MSG_REPAIR_REQUIRED);
    }
}

#[test]
fn recovery_ledger_records_only_terminal_results() {
    let ledger = RecoveryLedger::default();
    ledger.record("/a", &RecoveryResult::SkippedBusy);
    assert_eq!(ledger.get("/a"), None);

    ledger.record(
        "/a",
        &RecoveryResult::RestoreFailed {
            error: "boom".into(),
        },
    );
    let failed = ledger.get("/a").unwrap();
    assert_eq!(failed.outcome, RecoveryOutcome::Failed);
    assert_eq!(failed.message.as_deref(), Some("boom"));

    ledger.record("/a", &RecoveryResult::Clean);
    assert_eq!(ledger.get("/a"), Some(failed));
    ledger.record("/a", &RecoveryResult::Restored);
    assert_eq!(ledger.get("/a").unwrap().outcome, RecoveryOutcome::Restored);
    assert_eq!(ledger.get("/b"), None);
}

fn install_outcome(restore: PrefixVersionRestoreOutcome) -> PrefixDependencyInstallOutcome {
    PrefixDependencyInstallOutcome {
        install_succeeded: true,
        install_exit_code: Some(0),
        install_error: None,
        restore_state: restore.state,
        restore_error: restore.error,
    }
}

// Ported completion_payload_preserves_legacy_success_semantics.
#[test]
fn completion_payload_preserves_legacy_success_semantics() {
    let restored = completion_payload(
        "profile",
        "/games/example/pfx",
        install_outcome(succeeded()),
    );
    assert!(restored.succeeded && restored.install_succeeded);
    assert_eq!(restored.exit_code, Some(0));
    assert_eq!(restored.install_exit_code, Some(0));
    assert_eq!(restored.prefix_path, "/games/example/pfx");

    let degraded = completion_payload(
        "profile",
        "/games/example/pfx",
        install_outcome(PrefixVersionRestoreOutcome {
            state: PrefixVersionRestoreState::Failed,
            error: Some("restore timed out".into()),
        }),
    );
    assert!(!degraded.succeeded && degraded.install_succeeded);

    let intentional = completion_payload(
        "profile",
        "/games/example/pfx",
        install_outcome(PrefixVersionRestoreOutcome {
            state: PrefixVersionRestoreState::NotRequired,
            error: None,
        }),
    );
    assert!(intentional.succeeded);
}
