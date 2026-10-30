//! Restore-journal recovery tests (feature-spec AC 815-1: J1–J3, J4d/J4e,
//! live prefix) over the real metadata journal, the real core restorer and
//! the `fake-tricks` script. Resolution and decision tests live in
//! `journal_resolve.rs`.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crosshook_core::metadata::PrefixVersionRestoreJournalRow;
use crosshook_core::prefix_deps::{PrefixVersionRestoreOutcome, PrefixVersionRestoreState};
use tokio_util::sync::CancellationToken;

use super::super::journal::{PrefixVersionJournal, RecoveryResult};
use super::super::seams::MetadataJournalStore;
use super::super::{RecoveryLedger, RecoveryOutcome, MSG_REPAIR_REQUIRED};
use super::fakes::{CountingRestorer, FakeInUse, FixedRestorer};
use super::helpers::{
    pending_row, read_calls, set_mode, write_system_reg, Harness, ToolScript, WinVersion,
};
use crate::AppErrorKind;

fn working_tool(h: &Harness) -> String {
    h.tool(&ToolScript::default())
}

fn assert_restored(h: &Harness, journal: &PrefixVersionJournal) {
    assert!(
        h.row().is_none(),
        "row must be deleted after a verified restore"
    );
    let status = journal.status(&h.prefix).unwrap();
    assert!(!status.required);
    assert_eq!(status.state, None);
}

// J1a: crash after the install changed the version, before the restore.
#[tokio::test]
async fn j1a_startup_restores_changed_prefix_and_deletes_row() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, restorer) = h.journal(false);

    let result = journal
        .recover(&h.prefix, false, true, &CancellationToken::new())
        .await;

    assert_eq!(result, RecoveryResult::Restored);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert_eq!(restorer.count(), 1);
    assert_restored(&h, &journal);
}

// J1b: protontricks rows restore with their persisted App ID first.
#[tokio::test]
async fn j1b_protontricks_row_restores_with_app_id() {
    let h = Harness::new();
    let tool = working_tool(&h);
    let mut row = pending_row(&h.prefix, &tool);
    row.tool_type = "protontricks".to_string();
    row.steam_app_id = Some("12345".to_string());
    h.seed_row(&row);
    let (journal, _) = h.journal(false);

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::Restored
    );
    assert_eq!(read_calls(&h.calls), vec!["12345 -q win10"]);
    assert_restored(&h, &journal);
}

// J1c: pre-install recovery restores a pending row before the install runs.
#[tokio::test]
async fn j1c_pre_install_recovery_restores_pending_row() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, _) = h.journal(true);

    let ledger = RecoveryLedger::default();
    let result = journal
        .recover_before_install(&h.prefix, &ledger, &CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result, RecoveryResult::Restored);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert_restored(&h, &journal);
    assert_eq!(
        ledger.get(&h.prefix).map(|entry| entry.outcome),
        Some(RecoveryOutcome::Restored)
    );
}

// J2a: crash after the restore, before the delete: idempotent re-restore.
#[tokio::test]
async fn j2a_already_restored_prefix_is_verified_and_row_deleted() {
    let h = Harness::new();
    write_system_reg(h.pfx(), WinVersion::Win10);
    let tool = h.tool(&ToolScript {
        rewrite_reg_on_restore: false,
        ..ToolScript::default()
    });
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, restorer) = h.journal(false);

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::Restored
    );
    assert_eq!(restorer.count(), 1);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert_restored(&h, &journal);
}

// J2b: crash after record, before spawn: no install verb ever runs.
#[tokio::test]
async fn j2b_unchanged_prefix_restores_without_install_verbs() {
    let h = Harness::new();
    write_system_reg(h.pfx(), WinVersion::Win10);
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, _) = h.journal(false);

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::Restored
    );
    let calls = read_calls(&h.calls);
    assert_eq!(calls, vec!["-q win10"]);
    assert!(calls.iter().all(|call| !call.contains("dotnet48")));
    assert_restored(&h, &journal);
}

// J3a: a failing restore marks the row failed with a sanitized error.
#[tokio::test]
async fn j3a_failed_restore_marks_row_failed_with_sanitized_error() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript {
        restore_exit: 1,
        ..ToolScript::default()
    });
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, _) = h.journal(false);

    let RecoveryResult::RestoreFailed { error } = journal
        .recover(&h.prefix, false, true, &CancellationToken::new())
        .await
    else {
        panic!("expected RestoreFailed");
    };
    assert!(
        error.contains("failed") && error.contains("Some(1)"),
        "{error}"
    );
    assert!(!error.contains("/home/"), "{error}");
    let row = h.row().expect("failed restore keeps the row");
    assert_eq!(row.state, "failed");
    assert_eq!(row.last_error.as_deref(), Some(error.as_str()));
    let status = journal.status(&h.prefix).unwrap();
    assert!(status.required);
    assert_eq!(status.state, Some(PrefixVersionRestoreState::Failed));
}

// J3b: exit 0 without the version change fails verification.
#[tokio::test]
async fn j3b_verification_mismatch_marks_row_failed() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript {
        rewrite_reg_on_restore: false,
        ..ToolScript::default()
    });
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, _) = h.journal(false);

    let RecoveryResult::RestoreFailed { error } = journal
        .recover(&h.prefix, false, true, &CancellationToken::new())
        .await
    else {
        panic!("expected RestoreFailed");
    };
    assert!(error.contains("verification"), "{error}");
    assert_eq!(h.row().unwrap().state, "failed");
}

// J3c: a timed-out restore (reported by the restorer) is recorded as failed.
// The real 90 s timeout is core-internal; its reporting is covered by core's
// `restoration_timeout_is_reported_separately`.
#[tokio::test]
async fn j3c_timed_out_restore_marks_row_failed() {
    let h = Harness::new();
    h.seed_row(&pending_row(&h.prefix, "winetricks"));
    let timeout = "restoring Windows version win10 timed out after 90 seconds";
    let restorer = Arc::new(CountingRestorer::new(
        Arc::new(FixedRestorer(PrefixVersionRestoreOutcome {
            state: PrefixVersionRestoreState::Failed,
            error: Some(timeout.to_string()),
        })),
        &h.prefix,
    ));
    let journal = PrefixVersionJournal::new(
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        restorer.clone(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::RestoreFailed {
            error: timeout.to_string()
        }
    );
    assert_eq!(restorer.count(), 1);
    let row = h.row().unwrap();
    assert_eq!(row.state, "failed");
    assert!(row.last_error.unwrap().contains("timed out"));
}

// J3d: startup never retries a failed row.
#[tokio::test]
async fn j3d_failed_row_is_not_auto_retried() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    h.fail_row("restore timed out");
    let before = h.row().unwrap();
    let (journal, restorer) = h.journal(false);

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::SkippedFailed
    );
    assert_eq!(restorer.count(), 0);
    assert!(!h.calls.exists());
    assert_eq!(h.row().unwrap(), before);
}

// J3e: a failed row blocks the install with the repair-required conflict.
#[tokio::test]
async fn j3e_failed_row_blocks_install() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    h.fail_row("restore failed");
    let (journal, restorer) = h.journal(false);

    let error = journal
        .recover_before_install(
            &h.prefix,
            &RecoveryLedger::default(),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_REPAIR_REQUIRED);
    assert_eq!(restorer.count(), 0);
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(h.row().unwrap().state, "failed");
}

// A failed pre-install recovery replaces an earlier success in the ledger.
#[tokio::test]
async fn failed_pre_install_recovery_is_recorded_in_ledger() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript {
        restore_exit: 1,
        ..ToolScript::default()
    });
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, _) = h.journal(false);
    let ledger = RecoveryLedger::default();
    ledger.record(&h.prefix, &RecoveryResult::Restored);

    let error = journal
        .recover_before_install(&h.prefix, &ledger, &CancellationToken::new())
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Conflict);
    let entry = ledger.get(&h.prefix).unwrap();
    assert_eq!(entry.outcome, RecoveryOutcome::Failed);
    assert_eq!(entry.message, h.row().unwrap().last_error);
}

// J3f: explicit Repair retries a failed row.
#[tokio::test]
async fn j3f_explicit_repair_restores_failed_row() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    h.fail_row("restore failed");
    let (journal, _) = h.journal(false);

    assert_eq!(
        journal
            .recover(&h.prefix, true, false, &CancellationToken::new())
            .await,
        RecoveryResult::Restored
    );
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert_restored(&h, &journal);
}

// J4d: a row that vanished under the key after the restore is clean and is
// never deleted.
#[tokio::test]
async fn j4d_row_vanished_after_restore_is_clean() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    let store = h.recording_store();
    *store.vanish_after_loads.lock().unwrap() = Some(1);
    let journal = PrefixVersionJournal::new(
        store.clone(),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::Clean
    );
    assert_eq!(store.ops(), vec!["load".to_string(), "load".to_string()]);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
}

/// Seeds `row`, runs startup recovery and asserts the row was never executed
/// and is now `failed`.
async fn assert_corrupt_never_executes(h: &Harness, row: PrefixVersionRestoreJournalRow) {
    h.seed_row(&row);
    let (journal, restorer) = h.journal(false);
    let result = journal
        .recover(&h.prefix, false, true, &CancellationToken::new())
        .await;
    assert!(
        matches!(result, RecoveryResult::Corrupt { .. }),
        "{result:?} for {row:?}"
    );
    assert_eq!(restorer.count(), 0, "restorer ran for {row:?}");
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(h.row().unwrap().state, "failed");
}

// J4e: corrupt rows (app id, verb) are never executed (ported
// persisted_protontricks_app_id_must_be_positive_decimal).
#[tokio::test]
async fn j4e_corrupt_rows_never_execute() {
    for app_id in [None, Some(""), Some("0"), Some("abc"), Some("--help")] {
        let h = Harness::new();
        let mut row = pending_row(&h.prefix, &working_tool(&h));
        row.tool_type = "protontricks".to_string();
        row.steam_app_id = app_id.map(str::to_string);
        assert_corrupt_never_executes(&h, row).await;
    }
    for verb in ["-q", "dotnet48"] {
        let h = Harness::new();
        let mut row = pending_row(&h.prefix, &working_tool(&h));
        row.restore_verb = verb.to_string();
        assert_corrupt_never_executes(&h, row).await;
    }
}

// J4e (M4): invalid binary paths are never executed and mark the row failed.
#[tokio::test]
async fn j4e_invalid_binary_path_never_executes() {
    let relative = Harness::new();
    let tool = working_tool(&relative);
    let name = std::path::Path::new(&tool)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_corrupt_never_executes(&relative, pending_row(&relative.prefix, &name)).await;

    let missing = Harness::new();
    let path = missing
        .dir
        .path()
        .join("missing-tricks")
        .to_string_lossy()
        .into_owned();
    assert_corrupt_never_executes(&missing, pending_row(&missing.prefix, &path)).await;

    let not_executable = Harness::new();
    let tool = working_tool(&not_executable);
    set_mode(std::path::Path::new(&tool), 0o644);
    assert_corrupt_never_executes(&not_executable, pending_row(&not_executable.prefix, &tool))
        .await;
}

// S-6: a row whose prefix path is not the canonical `pfx` path (symlinked
// alias, the prefix root, a vanished prefix) is corrupt and never executed.
#[tokio::test]
async fn non_canonical_prefix_rows_never_execute() {
    let h = Harness::new();
    let alias_root = h.dir.path().join("alias");
    std::os::unix::fs::symlink(&h.prefix_root, &alias_root).unwrap();
    let tool = working_tool(&h);
    for stored in [
        alias_root.join("pfx"),
        h.prefix_root.clone(),
        h.dir.path().join("missing/pfx"),
    ] {
        let stored = stored.to_string_lossy().into_owned();
        h.seed_row(&pending_row(&stored, &tool));
        let (journal, restorer) = h.journal(false);

        let result = journal
            .recover(&stored, false, true, &CancellationToken::new())
            .await;

        assert!(
            matches!(result, RecoveryResult::Corrupt { .. }),
            "{result:?} for {stored}"
        );
        assert_eq!(restorer.count(), 0, "restorer ran for {stored}");
        assert!(read_calls(&h.calls).is_empty());
        let row = h
            .metadata
            .load_prefix_version_restore(&stored)
            .unwrap()
            .expect("the row stays");
        assert_eq!(row.state, "failed");
    }
}

// J4 live prefix: startup recovery leaves a prefix in use pending.
#[tokio::test]
async fn j4_live_prefix_is_skipped_and_left_pending() {
    let h = Harness::new();
    let tool = working_tool(&h);
    h.seed_row(&pending_row(&h.prefix, &tool));
    let in_use = Arc::new(FakeInUse::new(true, &h.prefix));
    let restorer = h.restorer();
    let journal = PrefixVersionJournal::new(
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        restorer.clone(),
        in_use.clone(),
    );

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::SkippedInUse
    );
    assert_eq!(in_use.calls.load(Ordering::SeqCst), 1);
    assert_eq!(restorer.count(), 0);
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(h.row().unwrap().state, "pending");

    // Explicit repair and pre-install recovery do not consult the probe.
    assert_eq!(
        journal
            .recover(&h.prefix, true, false, &CancellationToken::new())
            .await,
        RecoveryResult::Restored
    );
    assert_eq!(in_use.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unavailable_store_is_reported_and_blocks_install() {
    let h = Harness::new();
    let store = h.recording_store();
    store.available.store(false, Ordering::SeqCst);
    let journal = PrefixVersionJournal::new(
        store.clone(),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
    );

    assert_eq!(
        journal
            .recover(&h.prefix, false, true, &CancellationToken::new())
            .await,
        RecoveryResult::StoreUnavailable
    );
    assert!(store.ops().is_empty());
    let error = journal
        .recover_before_install(
            &h.prefix,
            &RecoveryLedger::default(),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
}
