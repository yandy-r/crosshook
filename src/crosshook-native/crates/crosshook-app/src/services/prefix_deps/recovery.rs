//! Startup recovery of interrupted Windows-version restores (D8): every
//! `pending` journal row is restored in its own job keyed on the prefix, so it
//! never races an install or repair of the same prefix.

use super::journal::RecoveryResult;
use super::{PrefixDepsService, PREFIX_RECOVERY_JOB_KIND};
use crate::services::run_blocking;
use crate::AppErrorKind;

impl PrefixDepsService {
    /// Restores every `pending` journal row; `failed` rows wait for an
    /// explicit repair.
    ///
    /// Each prefix is recovered in a `prefix-version-recovery` job holding the
    /// key of its normalized path; a prefix whose key is already held is
    /// reported [`RecoveryResult::SkippedBusy`] and left to the holder (an
    /// install recovers it first). A prefix used by a live process is skipped
    /// with its row left `pending`, and so is every prefix whose restore had
    /// not started when app shutdown cancelled its job. Every result is
    /// recorded in the session's recovery ledger. An unavailable or
    /// unreadable journal recovers nothing.
    pub(crate) async fn recover_on_startup(&self) -> Vec<(String, RecoveryResult)> {
        if !self.journal.is_available() {
            return Vec::new();
        }
        let journal = self.journal.clone();
        let rows = match run_blocking(move || journal.recovery_rows()).await {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(%error, "failed to list prefix repair journal rows");
                return Vec::new();
            }
        };
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let journal = self.journal.clone();
            let canonical = row.canonical.clone();
            let recovered = self
                .run_keyed(
                    PREFIX_RECOVERY_JOB_KIND,
                    &canonical,
                    move |cancel| async move { journal.recover_row(&row, &cancel).await },
                )
                .await;
            let result = match recovered {
                Ok(result) => result,
                Err(error) if error.kind() == AppErrorKind::Conflict => RecoveryResult::SkippedBusy,
                Err(error) => {
                    tracing::warn!(%error, "prefix recovery job did not run");
                    continue;
                }
            };
            self.recoveries.record(&canonical, &result);
            let cancelled = result == RecoveryResult::SkippedCancelled;
            results.push((canonical, result));
            if cancelled {
                // App shutdown: every later job would start cancelled too.
                break;
            }
        }
        results
    }
}
