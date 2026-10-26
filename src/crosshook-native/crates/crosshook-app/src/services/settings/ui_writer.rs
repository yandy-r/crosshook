//! Throttled writer for the `[ui]` settings table.
//!
//! The writer coalesces bursts of preference changes into a leading write
//! plus at most one trailing write per [`MIN_WRITE_INTERVAL`] window, and
//! applies only the dirty keys through the sink so concurrent settings
//! writes are never reverted.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshook_core::settings::{UiKey, UiKeySet, UiSettings};
use tokio::runtime::Handle;
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tokio_util::sync::{CancellationToken, DropGuard};

use crate::events::{AppEvent, EventBus};
use crate::AppError;

/// Minimum spacing between two `[ui]` writes: a leading write when idle,
/// then one trailing write for changes made inside the window.
pub(super) const MIN_WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// A pending `[ui]` write: the full in-memory snapshot plus the dirty keys.
pub(super) struct UiPatch {
    /// Snapshot the dirty keys are copied from.
    pub(super) settings: UiSettings,
    /// Keys that changed since the previous write.
    pub(super) keys: UiKeySet,
}

/// Future returned by a [`UiSink`].
pub(super) type SinkFuture = Pin<Box<dyn Future<Output = Result<(), AppError>> + Send>>;

/// Applies a [`UiPatch`] to the on-disk `[ui]` table.
pub(super) type UiSink = Arc<dyn Fn(UiPatch) -> SinkFuture + Send + Sync>;

/// Writer-internal state, guarded by a sync mutex that is never held across
/// an `.await`.
struct State {
    current: UiSettings,
    dirty: UiKeySet,
    failure_reported: bool,
}

/// State shared between [`UiWriter`] handles and the background task.
struct Shared {
    state: Mutex<State>,
    notify: Notify,
    gate: AsyncMutex<()>,
    sink: UiSink,
    events: EventBus,
}

/// Writes the pending dirty keys through the sink, serialized against other
/// writers by the async gate. Dirty keys are dropped even on failure; the
/// next real change re-arms them. The first failure of the writer's life
/// publishes [`AppEvent::UiSettingsPersistFailed`]; later failures only log.
async fn write_pending(shared: &Shared) -> Result<(), AppError> {
    let _write = shared.gate.lock().await;
    let (settings, keys) = {
        let mut state = shared.state.lock().expect("ui settings mutex poisoned");
        let keys = std::mem::take(&mut state.dirty);
        if keys.is_empty() {
            return Ok(());
        }
        (state.current.clone(), keys)
    };

    if let Err(error) = (shared.sink)(UiPatch { settings, keys }).await {
        let mut state = shared.state.lock().expect("ui settings mutex poisoned");
        tracing::error!(error = error.message(), "failed to persist [ui] settings");
        if !state.failure_reported {
            state.failure_reported = true;
            drop(state);
            shared.events.publish(AppEvent::UiSettingsPersistFailed {
                message: error.message().to_owned(),
            });
        }
        return Err(error);
    }
    Ok(())
}

/// Handle to the throttled `[ui]` writer task.
pub(super) struct UiWriter {
    shared: Arc<Shared>,
    /// Cancels the task when the last writer handle drops.
    _guard: DropGuard,
}

impl UiWriter {
    /// Spawns the writer task on `runtime`.
    ///
    /// The loop waits for a wake-up, writes the dirty keys immediately
    /// (leading edge), then sleeps for [`MIN_WRITE_INTERVAL`]. Changes made
    /// during the sleep leave a stored permit, producing exactly one
    /// trailing write. The task exits when the last [`UiWriter`] handle
    /// drops, which fires the cancellation token's [`DropGuard`].
    pub(super) fn spawn(
        initial: UiSettings,
        sink: UiSink,
        events: EventBus,
        runtime: &Handle,
    ) -> Self {
        let cancel = CancellationToken::new();
        let guard = cancel.clone().drop_guard();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                current: initial,
                dirty: UiKeySet::empty(),
                failure_reported: false,
            }),
            notify: Notify::new(),
            gate: AsyncMutex::new(()),
            sink,
            events,
        });

        let task_shared = Arc::clone(&shared);
        let task_cancel = cancel;
        runtime.spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = task_cancel.cancelled() => break,
                    _ = task_shared.notify.notified() => {}
                }
                let _ = write_pending(&task_shared).await;
                tokio::time::sleep(MIN_WRITE_INTERVAL).await;
            }
        });

        Self {
            shared,
            _guard: guard,
        }
    }

    /// Returns the current in-memory `[ui]` snapshot.
    pub(super) fn snapshot(&self) -> UiSettings {
        self.shared
            .state
            .lock()
            .expect("ui settings mutex poisoned")
            .current
            .clone()
    }

    /// Applies `apply` to the in-memory copy, normalizes it, and — when
    /// anything changed — marks `key` dirty and wakes the writer task.
    /// Returns whether the snapshot changed.
    pub(super) fn mutate(&self, key: UiKey, apply: impl FnOnce(&mut UiSettings)) -> bool {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("ui settings mutex poisoned");
        let mut updated = state.current.clone();
        apply(&mut updated);
        updated.normalize();
        if updated == state.current {
            return false;
        }
        state.current = updated;
        state.dirty.insert(key);
        drop(state);
        self.shared.notify.notify_one();
        true
    }

    /// Writes the pending dirty keys immediately. `Ok(())` when nothing is
    /// pending; the sink error otherwise.
    pub(super) async fn flush(&self) -> Result<(), AppError> {
        write_pending(&self.shared).await
    }
}
