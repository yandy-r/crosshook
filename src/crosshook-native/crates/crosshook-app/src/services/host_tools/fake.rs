//! Deterministic host probe shared by service tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crosshook_core::onboarding::{HostToolDetails, ReadinessCheckResult};

use super::HostProbe;

/// Canned host results with per-operation call counts and an optional gate.
pub(crate) struct FakeHostProbe {
    pub system: ReadinessCheckResult,
    pub generalized: ReadinessCheckResult,
    pub details: HostToolDetails,
    pub system_calls: AtomicUsize,
    pub generalized_calls: AtomicUsize,
    pub details_calls: AtomicUsize,
    gate: Option<Arc<GateState>>,
}

impl FakeHostProbe {
    /// Returns the supplied results without inspecting the live host.
    pub(crate) fn new(
        system: ReadinessCheckResult,
        generalized: ReadinessCheckResult,
        details: HostToolDetails,
    ) -> Self {
        Self {
            system,
            generalized,
            details,
            system_calls: AtomicUsize::new(0),
            generalized_calls: AtomicUsize::new(0),
            details_calls: AtomicUsize::new(0),
            gate: None,
        }
    }

    /// Blocks probe calls until the returned gate is released.
    pub(crate) fn gated(
        system: ReadinessCheckResult,
        generalized: ReadinessCheckResult,
        details: HostToolDetails,
    ) -> (Self, ProbeGate) {
        let gate = Arc::new(GateState {
            entered: tokio::sync::Notify::new(),
            released: Mutex::new(false),
            wake: Condvar::new(),
        });
        let mut probe = Self::new(system, generalized, details);
        probe.gate = Some(gate.clone());
        (probe, ProbeGate(gate))
    }

    fn wait_for_release(&self) {
        if let Some(gate) = &self.gate {
            gate.entered.notify_one();
            let released = gate.released.lock().unwrap();
            drop(
                gate.wake
                    .wait_while(released, |released| !*released)
                    .unwrap(),
            );
        }
    }
}

impl HostProbe for FakeHostProbe {
    fn system_readiness(&self) -> ReadinessCheckResult {
        self.system_calls.fetch_add(1, Ordering::SeqCst);
        self.wait_for_release();
        self.system.clone()
    }

    fn generalized_readiness(&self) -> ReadinessCheckResult {
        self.generalized_calls.fetch_add(1, Ordering::SeqCst);
        self.wait_for_release();
        self.generalized.clone()
    }

    fn tool_details(&self, _tool_id: &str) -> HostToolDetails {
        self.details_calls.fetch_add(1, Ordering::SeqCst);
        self.wait_for_release();
        self.details.clone()
    }
}

struct GateState {
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    wake: Condvar,
}

/// Releases blocked probes explicitly, or automatically on drop.
pub(crate) struct ProbeGate(Arc<GateState>);

impl ProbeGate {
    /// Waits asynchronously until a probe enters the blocking gate.
    pub(crate) async fn entered(&self) {
        self.0.entered.notified().await;
    }

    /// Allows all current and future probe calls to finish.
    pub(crate) fn release(&self) {
        *self.0.released.lock().unwrap() = true;
        self.0.wake.notify_all();
    }
}

impl Drop for ProbeGate {
    fn drop(&mut self) {
        self.release();
    }
}
