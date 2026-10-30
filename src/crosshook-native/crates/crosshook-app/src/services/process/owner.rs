//! Owner slots for the no-argument cancel/stop commands (D6).
//!
//! A service claims, per invoking owner (derived from the webview label), one
//! slot before it prepares a run; the job binds its id to the slot when its
//! body starts. Cancel and stop act only on the caller's own slot, so a
//! foreign owner or an owner without an active run never touches anyone
//! else's job. An owner holds at most one slot: a second start while its run
//! is still active is refused, so the first run never becomes unreachable.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio_util::sync::CancellationToken;

use crate::jobs::{JobId, JobRegistry};
use crate::{AppError, AppErrorKind};

/// Conflict text for a second start while the owner's run is still active.
pub(crate) const MSG_OWNER_BUSY: &str = "another run is already active for this window";

/// Who started a job, e.g. the invoking webview label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobOwner(String);

impl JobOwner {
    /// An owner identified by `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

/// One owner's slot: its job id once bound, a cancel requested before the
/// bind, and the run's immediate-kill token.
struct Slot {
    id: Option<JobId>,
    cancel_requested: bool,
    kill: CancellationToken,
}

type Slots = HashMap<JobOwner, Slot>;

/// Shared map from owner to its slot. Cheap to clone; clones share state.
#[derive(Clone, Default)]
pub(crate) struct ActiveJobs(Arc<Mutex<Slots>>);

fn lock(slots: &Mutex<Slots>) -> MutexGuard<'_, Slots> {
    // The lock is never held across user code, so a poisoned map is consistent.
    slots.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ActiveJobs {
    /// Claims `owner`'s slot until the returned guard drops.
    ///
    /// # Errors
    /// `Conflict` ([`MSG_OWNER_BUSY`]) while `owner` still holds a slot.
    pub(crate) fn claim(&self, owner: JobOwner) -> Result<OwnerSlot, AppError> {
        let mut slots = lock(&self.0);
        if slots.contains_key(&owner) {
            return Err(AppError::new(AppErrorKind::Conflict, MSG_OWNER_BUSY));
        }
        let kill = CancellationToken::new();
        slots.insert(
            owner.clone(),
            Slot {
                id: None,
                cancel_requested: false,
                kill: kill.clone(),
            },
        );
        Ok(OwnerSlot {
            slots: Arc::clone(&self.0),
            owner,
            kill,
        })
    }

    /// Cancels `owner`'s run: its job through `jobs` once bound, otherwise
    /// the run is marked so its job ends before spawning anything. Returns
    /// `false` when `owner` holds no slot.
    pub(crate) fn cancel(&self, owner: &JobOwner, jobs: &JobRegistry) -> bool {
        let id = {
            let mut slots = lock(&self.0);
            let Some(slot) = slots.get_mut(owner) else {
                return false;
            };
            match &slot.id {
                Some(id) => id.clone(),
                None => {
                    slot.cancel_requested = true;
                    return true;
                }
            }
        };
        jobs.cancel(&id).is_ok()
    }

    /// Fires `owner`'s immediate-kill token. Returns `false` when `owner`
    /// holds no slot.
    pub(crate) fn kill(&self, owner: &JobOwner) -> bool {
        match lock(&self.0).get(owner) {
            Some(slot) => {
                slot.kill.cancel();
                true
            }
            None => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock(&self.0).len()
    }
}

/// A claimed owner slot; dropping it — including while unwinding — frees the
/// owner for its next run.
pub(crate) struct OwnerSlot {
    slots: Arc<Mutex<Slots>>,
    owner: JobOwner,
    kill: CancellationToken,
}

impl OwnerSlot {
    /// The run's immediate-kill token (fired by [`ActiveJobs::kill`]).
    pub(crate) fn kill_token(&self) -> &CancellationToken {
        &self.kill
    }

    /// Binds the slot to the job `id`, so later cancels reach that job.
    /// Returns `true` when a cancel was requested before the bind: the job
    /// must then end without spawning anything.
    pub(crate) fn bind(&self, id: &JobId) -> bool {
        let mut slots = lock(&self.slots);
        let Some(slot) = slots.get_mut(&self.owner) else {
            return false;
        };
        slot.id = Some(id.clone());
        slot.cancel_requested
    }
}

impl Drop for OwnerSlot {
    fn drop(&mut self) {
        // Claims are exclusive per owner: the entry is this slot's own.
        lock(&self.slots).remove(&self.owner);
    }
}
