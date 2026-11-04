//! Real session-bus round trip for single-instance activation.
//!
//! Uses a per-process name (`dev.crosshook.CrossHookTest.p<pid>`) so a
//! running CrossHook is never touched. Without a session bus (CI) the test
//! prints a skip reason and returns; run it locally under
//! `dbus-run-session -- cargo test -p crosshook-app --test instance_activation`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshook_app::instance_lock::{
    activate_named, claim_bus_name_at, ActivationError, ActivationHandler, ActivationRequest,
    ActivationStatus, ClaimOutcome,
};
use tokio::sync::Notify;

#[derive(Default)]
struct Recorder {
    requests: Mutex<Vec<ActivationRequest>>,
    notify: Notify,
}

#[derive(Clone, Default)]
struct Shared(Arc<Recorder>);

impl ActivationHandler for Shared {
    fn activate(&self, request: ActivationRequest) -> ActivationStatus {
        self.0.requests.lock().unwrap().push(request);
        self.0.notify.notify_one();
        ActivationStatus::Activated
    }
}

/// A process that is not CrossHook but owns the name and implements the
/// interface with its own error.
struct Foreign;

#[zbus::interface(name = "org.freedesktop.Application")]
impl Foreign {
    fn activate(
        &self,
        _platform_data: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    ) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::Failed("not CrossHook".to_owned()))
    }
}

/// An instance whose window is gone (shutting down).
struct Closing;

impl ActivationHandler for Closing {
    fn activate(&self, _request: ActivationRequest) -> ActivationStatus {
        ActivationStatus::NoWindow
    }
}

fn token(value: &str) -> ActivationRequest {
    ActivationRequest {
        activation_token: Some(value.to_owned()),
        desktop_startup_id: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn activation_round_trip_over_the_session_bus() {
    match tokio::time::timeout(Duration::from_secs(2), zbus::Connection::session()).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            eprintln!("skip: no session bus: {error}");
            return;
        }
        Err(_) => {
            eprintln!("skip: no session bus: connect timed out");
            return;
        }
    }
    let name = format!("dev.crosshook.CrossHookTest.p{}", std::process::id());

    // 1. The first claim owns the name.
    let recorder = Shared::default();
    let ClaimOutcome::Owned(_claim) = claim_bus_name_at(&name, recorder.clone()).await else {
        panic!("first claim must own {name}");
    };

    // 2. Activate reaches the handler with the forwarded token.
    let notified = recorder.0.notify.notified();
    activate_named(&name, token("tok"), Duration::from_secs(2))
        .await
        .expect("activate the owner");
    tokio::time::timeout(Duration::from_secs(2), notified)
        .await
        .expect("handler called");
    assert_eq!(*recorder.0.requests.lock().unwrap(), vec![token("tok")]);

    // 3. A second claim sees the name taken: a second instance.
    assert!(matches!(
        claim_bus_name_at(&name, Shared::default()).await,
        ClaimOutcome::AlreadyOwned
    ));

    // 4. Nobody owns the name: bounded, reported as no owner.
    let none = format!("{name}.none");
    let started = tokio::time::Instant::now();
    let result = activate_named(&none, token("x"), Duration::from_millis(300)).await;
    assert!(
        matches!(result, Err(ActivationError::NoOwner)),
        "{result:?}"
    );
    assert!(started.elapsed() >= Duration::from_millis(300));

    // 5. Late owner: activation started before the name exists still lands.
    let late = format!("{name}.late");
    let late_recorder = Shared::default();
    let notified = late_recorder.0.notify.notified();
    let pending = tokio::spawn({
        let late = late.clone();
        async move { activate_named(&late, token("late"), Duration::from_secs(2)).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let ClaimOutcome::Owned(_late_claim) = claim_bus_name_at(&late, late_recorder.clone()).await
    else {
        panic!("late claim must own {late}");
    };
    pending
        .await
        .unwrap()
        .expect("activation waits for the owner");
    tokio::time::timeout(Duration::from_secs(2), notified)
        .await
        .expect("late handler called");
    assert_eq!(
        *late_recorder.0.requests.lock().unwrap(),
        vec![token("late")]
    );
    // 6. Owner without a window (shutting down): Activate is rejected.
    let closing = format!("{name}.closing");
    let ClaimOutcome::Owned(_closing_claim) = claim_bus_name_at(&closing, Closing).await else {
        panic!("closing claim must own {closing}");
    };
    let result = activate_named(&closing, token("x"), Duration::from_secs(2)).await;
    assert!(
        matches!(result, Err(ActivationError::ShuttingDown)),
        "{result:?}"
    );

    // 6b. An unrelated process owns the name and answers with its own error:
    // reported as a rejection with detail, not as a shutdown.
    let foreign = format!("{name}.foreign");
    let _foreign = zbus::connection::Builder::session()
        .unwrap()
        .serve_at("/dev/crosshook/CrossHook", Foreign)
        .unwrap()
        .name(foreign.as_str())
        .unwrap()
        .build()
        .await
        .expect("foreign owner");
    let result = activate_named(&foreign, token("x"), Duration::from_secs(2)).await;
    assert!(
        matches!(&result, Err(ActivationError::Rejected(detail)) if detail.contains("org.freedesktop.DBus.Error.Failed")),
        "{result:?}"
    );

    // 6c. The owner releases the name and a successor claims it before the
    // deadline: the activation waits for the successor and lands there.
    let moving = format!("{name}.moving");
    let ClaimOutcome::Owned(old) = claim_bus_name_at(&moving, Closing).await else {
        panic!("first owner must own {moving}");
    };
    drop(old); // closes the connection: the name has no owner now
    let successor = Shared::default();
    let notified = successor.0.notify.notified();
    let pending = tokio::spawn({
        let moving = moving.clone();
        async move { activate_named(&moving, token("moved"), Duration::from_secs(2)).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let ClaimOutcome::Owned(_successor_claim) = claim_bus_name_at(&moving, successor.clone()).await
    else {
        panic!("successor must own {moving}");
    };
    pending
        .await
        .unwrap()
        .expect("activation reaches the successor");
    tokio::time::timeout(Duration::from_secs(2), notified)
        .await
        .expect("successor handler called");

    // 7. The synchronous entry point refuses to run inside a runtime.
    let result = crosshook_app::instance_lock::activate_running_instance(
        token("x"),
        Duration::from_millis(10),
    );
    assert!(matches!(result, Err(ActivationError::Bus(_))), "{result:?}");

    println!("instance activation round trip: all cases passed on {name}");
}
