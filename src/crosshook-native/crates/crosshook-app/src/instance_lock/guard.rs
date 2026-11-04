//! Process-lifetime single-instance guard owned by the bootstrapped app.

use std::io;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::{ActivationHandler, ActivationRelay, BusClaim, InstanceLock, InstanceLockState};

/// Holds the lock and the bus name for the whole process, relays activations
/// to the UI, and hands the instance to another binary.
pub struct InstanceGuard {
    lock: Option<InstanceLock>,
    bus: Mutex<Option<BusClaim>>,
    relay: Arc<ActivationRelay>,
    unguarded: bool,
    handoff_pending: AtomicBool,
}

/// Failure of [`InstanceGuard::release_and_handoff`].
#[derive(Debug, thiserror::Error)]
pub enum HandoffError {
    /// This process does not hold the lock, so there is nothing to hand off.
    #[error("instance lock not held")]
    NotHeld,
    /// The other binary could not be started; the lock and the bus name are
    /// still held.
    #[error("could not start the other CrossHook: {0}")]
    Spawn(io::Error),
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl InstanceGuard {
    pub(crate) fn new(
        state: InstanceLockState,
        bus: Option<BusClaim>,
        relay: Arc<ActivationRelay>,
        unguarded: bool,
    ) -> Self {
        let lock = match state {
            InstanceLockState::Held(lock) => Some(lock),
            InstanceLockState::Unavailable(_) => None,
        };
        Self {
            lock,
            bus: Mutex::new(bus),
            relay,
            unguarded,
            handoff_pending: AtomicBool::new(false),
        }
    }

    /// Whether this process holds the runtime-dir lock. It stays held until
    /// the process exits, including after a handoff.
    pub fn holds_lock(&self) -> bool {
        self.lock.is_some()
    }

    /// Whether this process owns the `dev.crosshook.CrossHook` bus name.
    pub fn owns_bus_name(&self) -> bool {
        locked(&self.bus).is_some()
    }

    /// Whether nothing guards this process (`degraded.instance_unguarded`).
    pub fn is_unguarded(&self) -> bool {
        self.unguarded
    }

    /// Whether [`InstanceGuard::release_and_handoff`] started another binary
    /// that now waits for this process to exit.
    pub fn handoff_pending(&self) -> bool {
        self.handoff_pending.load(Ordering::Acquire)
    }

    /// Installs the UI's window-raising handler and delivers the latest
    /// activation that arrived before it existed.
    pub fn set_activation_handler(&self, handler: impl ActivationHandler) {
        self.relay.set_handler(Arc::new(handler));
    }

    /// Gives up the bus name (idempotent). Shells call it when they start
    /// exiting, so a relaunch during shutdown is told the instance is going
    /// away instead of activating a window that no longer exists.
    pub async fn release_bus_name(&self) {
        let claim = locked(&self.bus).take();
        if let Some(claim) = claim {
            claim.release().await;
        }
    }

    /// Hands the instance to another binary: spawns `cmd` with
    /// `CROSSHOOK_INSTANCE_HANDOFF=1`, then releases the bus name. The lock
    /// stays held until this process exits (the kernel releases it and the
    /// new process, waiting up to `HANDOFF_WAIT`, takes it), so the caller
    /// must exit right after `Ok`. On a spawn failure the lock and the bus
    /// name are both kept and nothing changes.
    pub async fn release_and_handoff(&self, cmd: Command) -> Result<Child, HandoffError> {
        let lock = self.lock.as_ref().ok_or(HandoffError::NotHeld)?;
        let child = lock.spawn_handoff(cmd).map_err(HandoffError::Spawn)?;
        self.handoff_pending.store(true, Ordering::Release);
        self.release_bus_name().await;
        Ok(child)
    }
}

static_assertions::assert_impl_all!(InstanceGuard: Send, Sync);
