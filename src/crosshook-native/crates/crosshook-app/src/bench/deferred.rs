//! The parked replay plan: with a scenario that defers the replay, READY stores the plan here
//! instead of handing it to the shell, and the shell takes it once after `bench_begin`.

use std::sync::Mutex;

use super::env::bench_enabled;
use super::plan::ReplayPlan;

static DEFERRED: DeferredReplay = DeferredReplay::new();

/// A one-shot slot for the replay plan.
pub(super) struct DeferredReplay(Mutex<Option<ReplayPlan>>);

impl DeferredReplay {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub(super) fn park(&self, plan: ReplayPlan) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(plan);
    }

    /// The parked plan, once: the slot is empty afterwards.
    pub(super) fn take(&self) -> Option<ReplayPlan> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

/// Park `plan` in the process-wide slot.
pub(super) fn park_deferred(plan: ReplayPlan) {
    DEFERRED.park(plan);
}

/// The replay plan READY parked for a scenario that defers the replay, handed out once. The
/// shell calls it after a successful `bench_begin` and then starts the replay through
/// [`replay_after_go`](super::replay_after_go). `None` unless `CROSSHOOK_BENCH=1` and a plan is
/// parked, so a normal process (and the G4 `console` scenario, whose replay starts at READY)
/// never has one.
pub fn take_deferred_replay() -> Option<ReplayPlan> {
    if !bench_enabled() {
        return None;
    }
    DEFERRED.take()
}

#[cfg(test)]
mod tests;
