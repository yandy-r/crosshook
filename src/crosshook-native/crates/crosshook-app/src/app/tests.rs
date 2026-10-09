//! Tests for [`App`] runtime ownership, spawning, and shutdown.

use std::future::Future;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

use tokio::runtime::Handle;

use super::App;
use crate::AppErrorKind;

/// Parks the current thread until woken — a minimal std-only executor proving
/// `App::spawn` futures need no ambient Tokio context.
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
fn owned_app_spawn_polls_on_std_executor() {
    let app = App::new().unwrap();
    let future = app.spawn(async { 40 + 2 });
    assert_eq!(block_on_park(future).unwrap(), 42);
}

#[tokio::test]
async fn borrowed_app_spawn_runs_on_host_runtime() {
    let app = App::with_handle(Handle::current());
    assert_eq!(app.spawn(async { 7 }).await.unwrap(), 7);
}

#[tokio::test]
async fn clones_share_one_event_bus() {
    let app = App::new().unwrap();
    let clone = app.clone();
    let mut subscriber = clone.subscribe();
    assert_eq!(
        app.events().publish(crate::AppEvent::Resync { missed: 5 }),
        1
    );
    assert_eq!(
        subscriber.recv().await,
        Some(crate::AppEvent::Resync { missed: 5 })
    );
}

#[test]
fn dropping_last_clone_cancels_and_drops_tasks() {
    /// Signals on drop through a std channel — no sleeps involved.
    struct DropGuard(mpsc::Sender<()>);
    impl Drop for DropGuard {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    let (started_tx, started_rx) = mpsc::channel();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let app = App::new().unwrap();
    let clone = app.clone();
    let future = app.spawn(async move {
        let _guard = DropGuard(dropped_tx);
        // Handshake: sent only after the guard exists inside the task.
        started_tx.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    drop(future);
    started_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("task must start and own its guard before any App clone drops");

    drop(app);
    assert!(
        matches!(
            dropped_rx.recv_timeout(Duration::from_millis(200)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "dropping a non-last clone must not drop the task"
    );

    drop(clone);
    dropped_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("task must drop once the last App clone is gone");
}

#[tokio::test]
async fn owned_app_drop_inside_tokio_does_not_panic_and_cancels() {
    let app = App::new().unwrap();
    let future = app.spawn(std::future::pending::<()>());
    drop(app);
    let error = future.await.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Cancelled);
}

#[tokio::test]
async fn borrowed_host_survives_app_drop() {
    let app = App::with_handle(Handle::current());
    let future = app.spawn(std::future::pending::<()>());
    drop(app);
    assert_eq!(future.await.unwrap_err().kind(), AppErrorKind::Cancelled);
    // Host runtime remains fully usable after the app is gone.
    assert_eq!(tokio::spawn(async { 42 }).await.unwrap(), 42);
}

#[tokio::test]
async fn spawned_task_error_propagates() {
    let app = App::new().unwrap();
    let outcome: Result<(), crate::AppError> = app
        .spawn(async {
            Err(crate::AppError::from(std::io::Error::from(
                std::io::ErrorKind::NotFound,
            )))
        })
        .await
        .unwrap();
    assert_eq!(outcome.unwrap_err().kind(), AppErrorKind::Io);
}

#[test]
fn spawn_wakes_std_executor_without_ambient_runtime() {
    // Mutex handoff proves cross-thread wake from the app runtime into the
    // std executor parked on the main test thread.
    let app = App::new().unwrap();
    let shared = Arc::new(Mutex::new(0u32));
    let moved = Arc::clone(&shared);
    let future = app.spawn(async move {
        tokio::task::yield_now().await;
        *moved.lock().unwrap() = 1;
    });
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "test must run without an ambient Tokio runtime"
    );
    block_on_park(future).unwrap();
    assert_eq!(*shared.lock().unwrap(), 1);
}

#[test]
fn owned_runtime_supports_timers() {
    let app = App::new().unwrap();
    let future = app.spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        9
    });
    assert_eq!(block_on_park(future).unwrap(), 9);
}
