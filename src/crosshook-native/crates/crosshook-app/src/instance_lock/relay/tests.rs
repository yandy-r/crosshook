use std::sync::Mutex as StdMutex;
use std::time::Instant;

use super::*;
use crate::instance_lock::{HandoffError, InstanceGuard, InstanceLockState};

#[derive(Default)]
struct Recorder(StdMutex<Vec<ActivationRequest>>);

impl ActivationHandler for Arc<Recorder> {
    fn activate(&self, request: ActivationRequest) -> ActivationStatus {
        self.0.lock().unwrap().push(request);
        ActivationStatus::Activated
    }
}

struct Fixed(ActivationStatus);

impl ActivationHandler for Fixed {
    fn activate(&self, _request: ActivationRequest) -> ActivationStatus {
        self.0
    }
}

fn request(token: &str) -> ActivationRequest {
    ActivationRequest {
        activation_token: Some(token.to_owned()),
        desktop_startup_id: None,
    }
}

fn relay() -> Arc<ActivationRelay> {
    Arc::new(ActivationRelay::new(Handle::current()))
}

#[tokio::test(flavor = "multi_thread")]
async fn buffered_request_is_answered_only_after_the_handler_runs() {
    let relay = relay();
    let waiting = tokio::spawn({
        let relay = Arc::clone(&relay);
        async move { relay.request(request("early")).await }
    });
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished(), "no reply before a handler exists");
    let recorder = Arc::new(Recorder::default());
    // Let the request reach the buffer before installing the handler.
    while relay.state().waiters.is_empty() {
        tokio::task::yield_now().await;
    }
    relay.set_handler(Arc::new(Arc::clone(&recorder)));
    assert_eq!(
        waiting.await.unwrap(),
        RelayOutcome::Handled(ActivationStatus::Activated)
    );
    assert_eq!(*recorder.0.lock().unwrap(), vec![request("early")]);
    assert_eq!(
        relay.request(request("late")).await,
        RelayOutcome::Handled(ActivationStatus::Activated)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn latest_buffered_request_wins_and_every_waiter_gets_the_status() {
    let relay = relay();
    let first = tokio::spawn({
        let relay = Arc::clone(&relay);
        async move { relay.request(request("first")).await }
    });
    let second = tokio::spawn({
        let relay = Arc::clone(&relay);
        async move { relay.request(request("second")).await }
    });
    while relay.state().waiters.len() < 2 {
        tokio::task::yield_now().await;
    }
    let recorder = Arc::new(Recorder::default());
    relay.set_handler(Arc::new(Arc::clone(&recorder)));
    let handled = RelayOutcome::Handled(ActivationStatus::Activated);
    assert_eq!(first.await.unwrap(), handled);
    assert_eq!(second.await.unwrap(), handled);
    assert_eq!(recorder.0.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn handler_status_is_passed_through() {
    let relay = relay();
    relay.set_handler(Arc::new(Fixed(ActivationStatus::NoWindow)));
    assert_eq!(
        relay.request(request("x")).await,
        RelayOutcome::Handled(ActivationStatus::NoWindow)
    );
    relay.set_handler(Arc::new(Fixed(ActivationStatus::RaiseFailed)));
    assert_eq!(
        relay.request(request("x")).await,
        RelayOutcome::Handled(ActivationStatus::RaiseFailed)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_guard_fails_waiting_requests() {
    let relay = relay();
    let guard = InstanceGuard::new(
        InstanceLockState::Unavailable("test"),
        None,
        Arc::clone(&relay),
        true,
    );
    let waiting = tokio::spawn({
        let relay = Arc::clone(&relay);
        async move { relay.request(request("x")).await }
    });
    while relay.state().waiters.is_empty() {
        tokio::task::yield_now().await;
    }
    drop(guard); // bootstrap failed after the bus name was claimed
    assert_eq!(waiting.await.unwrap(), RelayOutcome::Closed);
    assert_eq!(relay.request(request("y")).await, RelayOutcome::Closed);
}

#[tokio::test(flavor = "multi_thread")]
async fn request_without_handler_gives_up_after_the_pending_wait() {
    let relay = relay();
    let started = Instant::now();
    assert_eq!(
        relay.request(request("x")).await,
        RelayOutcome::StillStarting
    );
    assert!(started.elapsed() >= PENDING_WAIT);
}

#[tokio::test(flavor = "multi_thread")]
async fn guard_installs_handler_through_the_relay() {
    let relay = relay();
    let guard = InstanceGuard::new(
        InstanceLockState::Unavailable("test"),
        None,
        Arc::clone(&relay),
        true,
    );
    assert!(!guard.holds_lock() && !guard.owns_bus_name() && guard.is_unguarded());
    let recorder = Arc::new(Recorder::default());
    guard.set_activation_handler(Arc::clone(&recorder));
    assert_eq!(
        relay.request(request("x")).await,
        RelayOutcome::Handled(ActivationStatus::Activated)
    );
    let handoff = guard
        .release_and_handoff(std::process::Command::new("true"))
        .await;
    assert!(matches!(handoff, Err(HandoffError::NotHeld)));
}
