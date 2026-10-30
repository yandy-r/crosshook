//! Startup recovery of rows stored under a stale key (B2: a prefix moved
//! behind a symlink is re-keyed and restored; a vanished prefix fails),
//! cancellation before the restore, and off-executor blocking work (B3).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, ThreadId};

use tokio_util::sync::CancellationToken;

use super::super::journal::{PrefixVersionJournal, RecoveryResult, RecoveryRow};
use super::super::seams::{MetadataJournalStore, PrefixInUse};
use super::fakes::FakeInUse;
use super::helpers::{pending_row, read_calls, Harness, ToolScript};

/// Seeds a row under `<dir>/old/pfx`, where `<dir>/old` is a symlink left at
/// the old location of the moved prefix; returns the stored path.
fn seed_moved_row(h: &Harness, tool: &str) -> String {
    let old_root = h.dir.path().join("old");
    std::os::unix::fs::symlink(&h.prefix_root, &old_root).unwrap();
    let stored = old_root.join("pfx").to_string_lossy().into_owned();
    h.seed_row(&pending_row(&stored, tool));
    stored
}

fn stored_row(h: &Harness, stored: &str) -> Option<String> {
    h.metadata
        .load_prefix_version_restore(stored)
        .unwrap()
        .map(|row| row.state)
}

#[tokio::test]
async fn moved_prefix_row_is_rekeyed_and_restored_under_the_canonical_path() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let stored = seed_moved_row(&h, &tool);
    let (journal, restorer) = h.journal(false);

    let rows = journal.recovery_rows().unwrap();
    assert_eq!(
        rows,
        vec![RecoveryRow {
            stored: stored.clone(),
            canonical: h.prefix.clone(),
        }]
    );

    let result = journal
        .recover_row(&rows[0], &CancellationToken::new())
        .await;

    assert_eq!(result, RecoveryResult::Restored);
    assert_eq!(restorer.count(), 1);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert_eq!(stored_row(&h, &stored), None);
    assert!(h.row().is_none());
    assert!(!journal.status(&h.prefix).unwrap().required);
}

#[tokio::test]
async fn moved_failed_row_is_rekeyed_so_status_reports_it() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let stored = seed_moved_row(&h, &tool);
    h.metadata
        .mark_prefix_version_restore_failed(&stored, "restore timed out")
        .unwrap();
    let (journal, restorer) = h.journal(false);

    let rows = journal.recovery_rows().unwrap();
    assert_eq!(rows.len(), 1, "a stale-keyed failed row is listed");
    let result = journal
        .recover_row(&rows[0], &CancellationToken::new())
        .await;

    assert_eq!(result, RecoveryResult::SkippedFailed);
    assert_eq!(restorer.count(), 0);
    assert_eq!(stored_row(&h, &stored), None);
    let status = journal.status(&h.prefix).unwrap();
    assert!(status.required);
    assert_eq!(status.last_error.as_deref(), Some("restore timed out"));
}

#[tokio::test]
async fn vanished_prefix_row_is_marked_failed_and_never_executed() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let stored = h
        .dir
        .path()
        .join("missing/pfx")
        .to_string_lossy()
        .into_owned();
    h.seed_row(&pending_row(&stored, &tool));
    let journal = PrefixVersionJournal::new(
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &stored)),
    );

    let rows = journal.recovery_rows().unwrap();
    assert_eq!(
        rows[0].canonical, stored,
        "an unresolvable row keeps its key"
    );
    let result = journal
        .recover_row(&rows[0], &CancellationToken::new())
        .await;

    assert!(
        matches!(result, RecoveryResult::Corrupt { .. }),
        "{result:?}"
    );
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(stored_row(&h, &stored).as_deref(), Some("failed"));
}

#[tokio::test]
async fn cancelled_job_never_starts_the_restore() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    let (journal, restorer) = h.journal(false);
    let cancel = CancellationToken::new();
    cancel.cancel();

    let result = journal.recover(&h.prefix, false, true, &cancel).await;

    assert_eq!(result, RecoveryResult::SkippedCancelled);
    assert_eq!(restorer.count(), 0);
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(h.row().unwrap().state, "pending");
}

/// In-use probe recording the thread it ran on.
struct ThreadProbe {
    threads: Mutex<Vec<ThreadId>>,
    calls: AtomicUsize,
}

impl PrefixInUse for ThreadProbe {
    fn in_use(&self, _canonical_prefix: &Path) -> bool {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.threads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(thread::current().id());
        false
    }
}

// B3: the store, validation and `/proc` work runs off the executor thread.
#[tokio::test(flavor = "current_thread")]
async fn blocking_recovery_work_runs_off_the_executor() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    let probe = Arc::new(ThreadProbe {
        threads: Mutex::new(Vec::new()),
        calls: AtomicUsize::new(0),
    });
    let journal = PrefixVersionJournal::new(
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        h.restorer(),
        probe.clone(),
    );

    let result = journal
        .recover(&h.prefix, false, true, &CancellationToken::new())
        .await;

    assert_eq!(result, RecoveryResult::Restored);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    let threads = probe.threads.lock().unwrap().clone();
    assert_ne!(threads, vec![thread::current().id()]);
}
