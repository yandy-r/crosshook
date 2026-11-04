//! Delivers `Activate` requests to the UI's [`ActivationHandler`] and holds
//! each D-Bus reply until the handler has answered.
//!
//! The bus name is owned before any window exists, so requests that arrive
//! first are buffered (the latest request wins) and their callers wait, at
//! most [`PENDING_WAIT`], for the handler. When the guard is dropped (startup
//! failed) the relay closes and every waiter is answered "shutting down".

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::oneshot;

use super::bus::{ActivationHandler, ActivationRequest, ActivationStatus};

/// How long a buffered request waits for the UI to install its handler.
/// Below the 2 s activation budget of the second launch, so it gets an
/// answer instead of a timeout.
pub(crate) const PENDING_WAIT: Duration = Duration::from_millis(1500);
/// Most callers that may wait for the handler at once; more are refused.
const MAX_WAITERS: usize = 16;

/// What the relay tells the `Activate` caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelayOutcome {
    /// The handler reported this status.
    Handled(ActivationStatus),
    /// The UI did not install its handler within [`PENDING_WAIT`].
    StillStarting,
    /// The relay closed (startup failed or the instance is exiting).
    Closed,
    /// Too many callers are already waiting.
    Busy,
}

#[derive(Default)]
struct RelayState {
    handler: Option<Arc<dyn ActivationHandler>>,
    pending: Option<ActivationRequest>,
    waiters: Vec<oneshot::Sender<ActivationStatus>>,
    closed: bool,
}

/// Buffers activations until the UI installs its handler; see the module docs.
pub(crate) struct ActivationRelay {
    state: Mutex<RelayState>,
    runtime: Handle,
}

impl ActivationRelay {
    /// Creates a relay that runs handlers on `runtime`'s blocking pool.
    pub(crate) fn new(runtime: Handle) -> Self {
        Self {
            state: Mutex::new(RelayState::default()),
            runtime,
        }
    }

    fn state(&self) -> MutexGuard<'_, RelayState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Delivers `request` and waits for the handler's answer (or for the
    /// handler to be installed, bounded by [`PENDING_WAIT`]).
    pub(crate) async fn request(&self, request: ActivationRequest) -> RelayOutcome {
        let receiver = match self.enqueue(request) {
            Ok(receiver) => receiver,
            Err(Ready::Run(handler, request)) => {
                return RelayOutcome::Handled(run_blocking(&self.runtime, handler, request).await);
            }
            Err(Ready::Done(outcome)) => return outcome,
        };
        match tokio::time::timeout(PENDING_WAIT, receiver).await {
            Ok(Ok(status)) => RelayOutcome::Handled(status),
            Ok(Err(_)) => RelayOutcome::Closed,
            Err(_) => RelayOutcome::StillStarting,
        }
    }

    /// Buffers `request` (returning the waiter) unless it can be answered or
    /// run right away. The lock is never held across an await.
    fn enqueue(
        &self,
        request: ActivationRequest,
    ) -> Result<oneshot::Receiver<ActivationStatus>, Ready> {
        let mut state = self.state();
        if state.closed {
            return Err(Ready::Done(RelayOutcome::Closed));
        }
        if let Some(handler) = state.handler.clone() {
            return Err(Ready::Run(handler, request));
        }
        if state.waiters.len() >= MAX_WAITERS {
            return Err(Ready::Done(RelayOutcome::Busy));
        }
        let (sender, receiver) = oneshot::channel();
        state.waiters.push(sender);
        state.pending = Some(request);
        Ok(receiver)
    }

    /// Installs the handler; the latest buffered request is handled on the
    /// blocking pool and every waiting caller gets its status.
    pub(crate) fn set_handler(&self, handler: Arc<dyn ActivationHandler>) {
        let (pending, waiters) = {
            let mut state = self.state();
            if state.closed {
                return;
            }
            state.handler = Some(Arc::clone(&handler));
            (state.pending.take(), std::mem::take(&mut state.waiters))
        };
        let Some(request) = pending else { return };
        let runtime = self.runtime.clone();
        self.runtime.spawn(async move {
            let status = run_blocking(&runtime, handler, request).await;
            for waiter in waiters {
                let _ = waiter.send(status);
            }
        });
    }

    /// Refuses all further requests and answers waiting callers "closed".
    pub(crate) fn close(&self) {
        let mut state = self.state();
        state.closed = true;
        state.handler = None;
        state.pending = None;
        state.waiters.clear();
    }
}

/// A request that does not need to wait for the handler.
enum Ready {
    Run(Arc<dyn ActivationHandler>, ActivationRequest),
    Done(RelayOutcome),
}

/// Handlers may block briefly (waiting for the UI thread), so they run on
/// the blocking pool; a panicking handler counts as a failed raise.
async fn run_blocking(
    runtime: &Handle,
    handler: Arc<dyn ActivationHandler>,
    request: ActivationRequest,
) -> ActivationStatus {
    runtime
        .spawn_blocking(move || handler.activate(request))
        .await
        .unwrap_or(ActivationStatus::RaiseFailed)
}

#[cfg(test)]
mod tests;
