//! Ledger tying each running install job to the caller that started it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::messages::MSG_OP_NOT_FOUND;
use super::types::InstallOwner;
use crate::jobs::JobId;
use crate::{AppError, AppErrorKind};

type Entries = HashMap<JobId, InstallOwner>;

/// Shared map from install job id to its [`InstallOwner`].
///
/// Cancel is owner-checked against this ledger, so a job id that is not in it
/// (unknown, foreign or belonging to an unrelated job) never reaches the job
/// registry.
#[derive(Clone, Default)]
pub(super) struct OwnerLedger(Arc<Mutex<Entries>>);

fn lock(entries: &Mutex<Entries>) -> MutexGuard<'_, Entries> {
    // The lock is never held across user code, so a poisoned map is consistent.
    entries.lock().unwrap_or_else(PoisonError::into_inner)
}

impl OwnerLedger {
    /// Records `owner` for `id` until the returned guard drops.
    pub(super) fn register(&self, id: JobId, owner: InstallOwner) -> OwnerGuard {
        lock(&self.0).insert(id.clone(), owner);
        OwnerGuard {
            entries: Arc::clone(&self.0),
            id,
        }
    }

    /// The job id for `op_id` when `owner` started it, leaving the entry in
    /// place (see [`claim`](Self::claim) for the consuming variant).
    ///
    /// # Errors
    /// `NotFound` with [`MSG_OP_NOT_FOUND`] for unknown and foreign ids alike,
    /// so the answer is no existence oracle.
    #[cfg(test)]
    pub(super) fn authorize(&self, owner: &InstallOwner, op_id: &str) -> Result<JobId, AppError> {
        let id = JobId::from_string(op_id);
        match lock(&self.0).get(&id) {
            Some(recorded) if recorded == owner => Ok(id),
            _ => Err(AppError::new(AppErrorKind::NotFound, MSG_OP_NOT_FOUND)),
        }
    }

    /// Like [`authorize`](Self::authorize), but also removes the entry, so a
    /// repeated cancel of the same op is `NotFound`.
    ///
    /// # Errors
    /// `NotFound` with [`MSG_OP_NOT_FOUND`], as for `authorize`.
    pub(super) fn claim(&self, owner: &InstallOwner, op_id: &str) -> Result<JobId, AppError> {
        let id = JobId::from_string(op_id);
        let mut entries = lock(&self.0);
        match entries.get(&id) {
            Some(recorded) if recorded == owner => {
                entries.remove(&id);
                Ok(id)
            }
            _ => Err(AppError::new(AppErrorKind::NotFound, MSG_OP_NOT_FOUND)),
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        lock(&self.0).len()
    }
}

/// Removes its ledger entry on drop, including while unwinding from a panic.
pub(super) struct OwnerGuard {
    entries: Arc<Mutex<Entries>>,
    id: JobId,
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        // The entry may already be gone (a cancel claims it first).
        lock(&self.entries).remove(&self.id);
    }
}
