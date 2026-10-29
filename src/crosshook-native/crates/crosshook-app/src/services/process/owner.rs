//! Owner slots for the no-argument cancel/stop commands (D6).
//!
//! A service records, per invoking owner (derived from the webview label),
//! the id of its active job and that job's immediate-kill token. Cancel and
//! stop act only on the caller's own entry, so a foreign owner or an owner
//! without an active job never touches anyone else's job.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio_util::sync::CancellationToken;

use crate::jobs::JobId;

/// Who started a job, e.g. the invoking webview label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobOwner(String);

impl JobOwner {
    /// An owner identified by `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

/// One owner's active job: its id and its immediate-kill token.
pub(crate) struct ActiveJob {
    pub(crate) id: JobId,
    pub(crate) kill: CancellationToken,
}

type Entries = HashMap<JobOwner, ActiveJob>;

/// Shared map from owner to its active job. Cheap to clone; clones share
/// state.
#[derive(Clone, Default)]
pub(crate) struct ActiveJobs(Arc<Mutex<Entries>>);

fn lock(entries: &Mutex<Entries>) -> MutexGuard<'_, Entries> {
    // The lock is never held across user code, so a poisoned map is consistent.
    entries.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ActiveJobs {
    /// Records `job` as `owner`'s active job, replacing any previous entry,
    /// until the returned guard drops.
    pub(crate) fn register(&self, owner: JobOwner, job: ActiveJob) -> ActiveGuard {
        let id = job.id.clone();
        lock(&self.0).insert(owner.clone(), job);
        ActiveGuard {
            entries: Arc::clone(&self.0),
            owner,
            id,
        }
    }

    /// `owner`'s active job id and kill token, if any.
    pub(crate) fn get(&self, owner: &JobOwner) -> Option<(JobId, CancellationToken)> {
        lock(&self.0)
            .get(owner)
            .map(|job| (job.id.clone(), job.kill.clone()))
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock(&self.0).len()
    }
}

/// Removes its owner's entry on drop — including while unwinding — but only
/// while that entry still belongs to the job that registered it, so a newer
/// job of the same owner is never unregistered by an older one finishing.
pub(crate) struct ActiveGuard {
    entries: Arc<Mutex<Entries>>,
    owner: JobOwner,
    id: JobId,
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let mut entries = lock(&self.entries);
        if entries
            .get(&self.owner)
            .is_some_and(|job| job.id == self.id)
        {
            entries.remove(&self.owner);
        }
    }
}
