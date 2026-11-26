//! zbus must work on a thread with no Tokio runtime (YAN-2475).
//!
//! Cargo unifies zbus features across a binary, so whichever backend
//! `crosshook-core`/`crosshook-app` pick is the one every zbus user in the
//! process gets: native UI toolkits, ashpd and `accesskit_unix` open D-Bus
//! connections on their own (non-Tokio) threads. With zbus's `tokio` feature
//! those panic ("there is no reactor running"). The workspace uses `async-io`
//! instead; `scripts/check-app-deps.sh` guards the feature, and this test
//! proves the behaviour on a real session bus.
//!
//! Without `DBUS_SESSION_BUS_ADDRESS` the test prints a skip reason and returns.
//! With it set (CI runs it under `dbus-run-session`), an unreachable or wedged
//! bus fails the test, so the path cannot pass vacuously.

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

/// Parks the current thread until woken: a std-only executor standing in for
/// a UI toolkit's own executor thread (no ambient Tokio context).
struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on_park<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

#[test]
fn session_bus_call_works_without_a_tokio_runtime() {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skip: DBUS_SESSION_BUS_ADDRESS is not set");
        return;
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "the test thread must have no Tokio runtime"
        );
        let outcome = block_on_park(async {
            let connection = zbus::Connection::session().await?;
            let proxy = zbus::fdo::DBusProxy::new(&connection).await?;
            let id = proxy.get_id().await?;
            Ok::<_, zbus::Error>(id.to_string())
        });
        let _ = sender.send(outcome);
    });
    // zbus's handshake has no timeout of its own; a wedged bus skips.
    let outcome = match receiver.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(outcome) => outcome,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            panic!("session bus did not answer within 5 s");
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            worker
                .join()
                .expect("zbus must not panic on a thread without a Tokio runtime");
            unreachable!("the worker exited without sending a result");
        }
    };

    let id = outcome.expect("session-bus call from a non-Tokio thread");
    assert!(!id.is_empty(), "bus id must be non-empty");
}
