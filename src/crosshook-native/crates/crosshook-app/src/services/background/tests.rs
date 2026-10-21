//! Background grant holder state transitions driven by a fake portal.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{
    BackgroundGrantHandle, BackgroundGrantHolder, BackgroundPortal, BackgroundProtectionState,
    BackgroundRequestError, BackgroundRequestResult, BACKGROUND_REQUEST_REASON,
};

/// Grant handle that records its drop.
#[derive(Debug)]
struct FakeGrant(Arc<Mutex<bool>>);

impl Drop for FakeGrant {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = true;
    }
}

/// Outcome a [`FakePortal`] returns from `request`.
#[derive(Debug, Clone)]
enum Outcome {
    Grant,
    Fail(BackgroundRequestError),
}

#[derive(Debug)]
struct FakePortal {
    supported: bool,
    outcome: Outcome,
    released: Arc<Mutex<bool>>,
    requests: Mutex<Vec<(String, bool)>>,
}

impl FakePortal {
    fn new(supported: bool, outcome: Outcome) -> Self {
        Self {
            supported,
            outcome,
            released: Arc::new(Mutex::new(false)),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl BackgroundPortal for FakePortal {
    fn supported(&self) -> bool {
        self.supported
    }

    async fn request(&self, reason: &str, autostart: bool) -> BackgroundRequestResult {
        self.requests
            .lock()
            .unwrap()
            .push((reason.to_string(), autostart));
        match &self.outcome {
            Outcome::Grant => {
                Ok(Box::new(FakeGrant(Arc::clone(&self.released)))
                    as Box<dyn BackgroundGrantHandle>)
            }
            Outcome::Fail(error) => Err(error.clone()),
        }
    }
}

#[test]
fn unsupported_portal_starts_not_applicable_and_initialized() {
    let holder = BackgroundGrantHolder::for_portal(&FakePortal::new(false, Outcome::Grant));
    assert_eq!(
        holder.protection_state(),
        BackgroundProtectionState::NotApplicable
    );
    assert!(holder.is_initialized());
    assert!(!holder.has_active_grant());
}

#[tokio::test]
async fn request_and_store_transitions_table() {
    let cases = [
        (Outcome::Grant, BackgroundProtectionState::Available, true),
        (
            Outcome::Fail(BackgroundRequestError::PortalDenied),
            BackgroundProtectionState::Degraded,
            false,
        ),
        (
            Outcome::Fail(BackgroundRequestError::Transport("bus down".to_string())),
            BackgroundProtectionState::Unavailable,
            false,
        ),
        (
            Outcome::Fail(BackgroundRequestError::AlreadyRequested),
            BackgroundProtectionState::Unavailable,
            false,
        ),
        (
            Outcome::Fail(BackgroundRequestError::NotSandboxed),
            BackgroundProtectionState::NotApplicable,
            false,
        ),
    ];
    for (outcome, expected, active) in cases {
        let portal = FakePortal::new(true, outcome.clone());
        let holder = BackgroundGrantHolder::for_portal(&portal);
        assert_eq!(
            holder.protection_state(),
            BackgroundProtectionState::Pending,
            "{outcome:?}"
        );
        assert!(!holder.is_initialized());

        holder.request_and_store(&portal).await;

        assert_eq!(holder.protection_state(), expected, "{outcome:?}");
        assert_eq!(holder.has_active_grant(), active, "{outcome:?}");
        assert!(holder.is_initialized());
        assert_eq!(
            *portal.requests.lock().unwrap(),
            vec![(BACKGROUND_REQUEST_REASON.to_string(), false)]
        );
    }
}

#[tokio::test]
async fn dropping_holder_releases_grant() {
    let portal = FakePortal::new(true, Outcome::Grant);
    let holder = BackgroundGrantHolder::for_portal(&portal);
    holder.request_and_store(&portal).await;
    assert!(!*portal.released.lock().unwrap());
    drop(holder);
    assert!(*portal.released.lock().unwrap());
}

/// A contract-violating re-request must not drop a held grant.
#[tokio::test]
async fn already_requested_after_grant_keeps_grant_and_state() {
    let portal = FakePortal::new(true, Outcome::Grant);
    let holder = BackgroundGrantHolder::for_portal(&portal);
    holder.request_and_store(&portal).await;
    assert_eq!(
        holder.protection_state(),
        BackgroundProtectionState::Available
    );

    holder.store_result(Err(BackgroundRequestError::AlreadyRequested));

    assert_eq!(
        holder.protection_state(),
        BackgroundProtectionState::Available
    );
    assert!(holder.has_active_grant());
    assert!(holder.is_initialized());
    assert!(
        !*portal.released.lock().unwrap(),
        "grant must not be dropped"
    );
}

#[tokio::test]
async fn wait_for_initialization_returns_immediately_when_unsupported() {
    let holder = BackgroundGrantHolder::for_portal(&FakePortal::new(false, Outcome::Grant));
    let state = holder.wait_for_initialization(Duration::from_secs(5)).await;
    assert_eq!(state, BackgroundProtectionState::NotApplicable);
}

#[tokio::test]
async fn wait_for_initialization_times_out_while_pending() {
    let holder = BackgroundGrantHolder::for_portal(&FakePortal::new(true, Outcome::Grant));
    let state = holder
        .wait_for_initialization(Duration::from_millis(10))
        .await;
    assert_eq!(state, BackgroundProtectionState::Pending);
}

/// Covers the race window where a waiter has subscribed to `Notify` before
/// `store_result` fires.
#[tokio::test]
async fn wait_for_initialization_unblocks_when_store_result_fires() {
    let portal = FakePortal::new(true, Outcome::Grant);
    let holder = Arc::new(BackgroundGrantHolder::for_portal(&portal));
    let waiter = {
        let holder = Arc::clone(&holder);
        tokio::spawn(async move { holder.wait_for_initialization(Duration::from_secs(5)).await })
    };
    // Let the waiter reach `Notified::enable()` before resolving.
    tokio::time::sleep(Duration::from_millis(50)).await;
    holder.store_result(Err(BackgroundRequestError::PortalDenied));
    let state = waiter.await.expect("waiter task must not panic");
    assert_eq!(state, BackgroundProtectionState::Degraded);
    assert!(holder.is_initialized());
    assert!(!holder.has_active_grant());
}

#[test]
fn protection_state_serializes_snake_case() {
    let json = serde_json::to_string(&BackgroundProtectionState::NotApplicable).unwrap();
    assert_eq!(json, "\"not_applicable\"");
}
