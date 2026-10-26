//! Tests for the throttled `[ui]` writer.
//!
//! All tests run with Tokio time paused; the sink is a pure async closure
//! that records `(Instant, UiPatch)` pairs, so no wall clock or blocking I/O
//! is involved.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crosshook_core::settings::{UiKey, UiSettings};
use tokio::runtime::Handle;
use tokio::time::{self, Duration, Instant};

use super::ui_writer::{SinkFuture, UiPatch, UiSink, UiWriter, MIN_WRITE_INTERVAL};
use crate::events::{AppEvent, EventBus, EventReceiver};
use crate::AppError;

/// Recorded sink call: when it ran and with which patch.
type Record = (Instant, UiPatch);

/// Collects the `UiSettingsPersistFailed` events published so far. The
/// virtual timeout terminates the drain once no more events arrive.
async fn persist_failures(receiver: &mut EventReceiver) -> usize {
    let mut failures = 0;
    loop {
        match time::timeout(Duration::from_secs(5), receiver.recv()).await {
            Ok(Some(event)) => {
                if matches!(event, AppEvent::UiSettingsPersistFailed { .. }) {
                    failures += 1;
                }
            }
            Ok(None) => return failures,
            Err(_elapsed) => return failures,
        }
    }
}

/// A recording sink that always succeeds.
fn recording_sink(count: Arc<AtomicUsize>, calls: Arc<Mutex<Vec<Record>>>) -> UiSink {
    Arc::new(move |patch: UiPatch| {
        count.fetch_add(1, Ordering::SeqCst);
        calls.lock().unwrap().push((Instant::now(), patch));
        Box::pin(async { Ok(()) }) as SinkFuture
    })
}

/// A sink that always fails.
fn failing_sink(count: Arc<AtomicUsize>) -> UiSink {
    Arc::new(move |_patch: UiPatch| {
        count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(AppError::validation("ui", "settings store is unwritable")) })
    })
}

/// A sink whose failures are controlled by `fail`, recording every call.
fn flaky_sink(fail: Arc<AtomicBool>, calls: Arc<Mutex<Vec<Result<Record, ()>>>>) -> UiSink {
    Arc::new(move |patch: UiPatch| {
        let fail = Arc::clone(&fail);
        let calls = Arc::clone(&calls);
        Box::pin(async move {
            if fail.load(Ordering::SeqCst) {
                calls.lock().unwrap().push(Err(()));
                return Err(AppError::validation("ui", "simulated store failure"));
            }
            calls.lock().unwrap().push(Ok((Instant::now(), patch)));
            Ok(())
        }) as SinkFuture
    })
}

/// Yields until recorded calls settle.
async fn settle(calls: &Arc<Mutex<Vec<Record>>>, expected: usize) {
    for _ in 0..200 {
        if calls.lock().unwrap().len() >= expected {
            return;
        }
        tokio::task::yield_now().await;
    }
}

fn set_window(ui: &mut UiSettings, width: u32, height: u32) {
    ui.window_width = Some(width);
    ui.window_height = Some(height);
}

#[tokio::test(start_paused = true)]
async fn hundred_resizes_collapse_into_at_most_two_writes() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let writer = UiWriter::spawn(
        UiSettings::default(),
        recording_sink(Arc::clone(&count), Arc::clone(&calls)),
        EventBus::default(),
        &Handle::current(),
    );

    for i in 0..100u32 {
        writer.mutate(UiKey::WindowSize, |ui| set_window(ui, 800 + i, 600 + i));
        time::advance(Duration::from_millis(5)).await;
    }
    time::advance(Duration::from_secs(2)).await;
    settle(&calls, usize::MAX).await;

    let calls = calls.lock().unwrap();
    assert!(
        calls.len() <= 2,
        "100 resizes inside one window must not exceed 2 writes, got {}",
        calls.len()
    );
    let Some((_, last)) = calls.last() else {
        panic!("the trailing write must have run");
    };
    assert_eq!(last.settings.window_width, Some(899));
    assert_eq!(last.settings.window_height, Some(699));
    assert!(last.keys.contains(UiKey::WindowSize));
    for pair in calls.windows(2) {
        let gap = pair[1].0 - pair[0].0;
        assert!(
            gap >= MIN_WRITE_INTERVAL,
            "consecutive writes must be spaced by at least {MIN_WRITE_INTERVAL:?}, got {gap:?}"
        );
    }
    drop(writer);
}

#[tokio::test(start_paused = true)]
async fn leading_write_is_immediate() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let writer = UiWriter::spawn(
        UiSettings::default(),
        recording_sink(Arc::clone(&count), Arc::clone(&calls)),
        EventBus::default(),
        &Handle::current(),
    );

    let before = Instant::now();
    writer.mutate(UiKey::SidebarCollapsed, |ui| ui.sidebar_collapsed = true);
    settle(&calls, 1).await;

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "an idle writer writes without waiting");
    assert_eq!(calls[0].0, before, "the leading write happens at once");
    drop(writer);
}

#[tokio::test(start_paused = true)]
async fn no_op_mutation_writes_nothing() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let writer = UiWriter::spawn(
        UiSettings::default(),
        recording_sink(Arc::clone(&count), Arc::clone(&calls)),
        EventBus::default(),
        &Handle::current(),
    );

    let changed = writer.mutate(UiKey::UiScale, |ui| ui.ui_scale = 1.0);
    assert!(!changed, "setting the current value is a no-op");
    time::advance(Duration::from_secs(3)).await;
    settle(&calls, usize::MAX).await;
    assert_eq!(count.load(Ordering::SeqCst), 0, "no sink call may happen");
    drop(writer);
}

#[tokio::test(start_paused = true)]
async fn flush_writes_pending_keys_and_is_quiet_when_idle() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let writer = UiWriter::spawn(
        UiSettings::default(),
        recording_sink(Arc::clone(&count), Arc::clone(&calls)),
        EventBus::default(),
        &Handle::current(),
    );

    writer.mutate(UiKey::LibraryViewMode, |ui| {
        ui.library_view_mode = Some(crosshook_core::settings::LibraryViewMode::List);
    });
    writer.mutate(UiKey::ConsoleDrawerHeight, |ui| {
        ui.console_drawer_height = 400;
    });
    writer.flush().await.expect("flush must succeed");

    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "flush writes the pending keys at once");
        assert!(calls[0].1.keys.contains(UiKey::LibraryViewMode));
        assert!(calls[0].1.keys.contains(UiKey::ConsoleDrawerHeight));
    }

    let before = count.load(Ordering::SeqCst);
    writer.flush().await.expect("second flush succeeds");
    assert_eq!(
        count.load(Ordering::SeqCst),
        before,
        "a flush with nothing pending must not call the sink"
    );
    drop(writer);
}

#[tokio::test(start_paused = true)]
async fn separate_windows_write_only_their_dirty_keys() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let writer = UiWriter::spawn(
        UiSettings::default(),
        recording_sink(Arc::clone(&count), Arc::clone(&calls)),
        EventBus::default(),
        &Handle::current(),
    );

    writer.mutate(UiKey::Density, |ui| {
        ui.density = crosshook_core::settings::UiDensity::Compact;
    });
    settle(&calls, 1).await;
    time::advance(MIN_WRITE_INTERVAL + Duration::from_millis(50)).await;

    writer.mutate(UiKey::InspectorWidth, |ui| ui.inspector_width = 480);
    settle(&calls, 2).await;

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    let keys: Vec<UiKey> = calls[0].1.keys.iter().collect();
    assert_eq!(
        keys,
        vec![UiKey::Density],
        "first write touches only Density"
    );
    let keys: Vec<UiKey> = calls[1].1.keys.iter().collect();
    assert_eq!(
        keys,
        vec![UiKey::InspectorWidth],
        "second write touches only InspectorWidth"
    );
    drop(writer);
}

#[tokio::test(start_paused = true)]
async fn first_failure_publishes_once_and_keeps_memory() {
    let count = Arc::new(AtomicUsize::new(0));
    let events = EventBus::default();
    let mut receiver = events.subscribe();
    let writer = UiWriter::spawn(
        UiSettings::default(),
        failing_sink(Arc::clone(&count)),
        events,
        &Handle::current(),
    );

    writer.mutate(UiKey::UiScale, |ui| ui.ui_scale = 1.5);
    writer
        .flush()
        .await
        .expect_err("first failing write errors");
    writer.mutate(UiKey::UiScale, |ui| ui.ui_scale = 0.8);
    writer
        .flush()
        .await
        .expect_err("second failing write errors");
    writer.mutate(UiKey::UiScale, |ui| ui.ui_scale = 1.25);
    writer
        .flush()
        .await
        .expect_err("third failing write errors");

    assert_eq!(count.load(Ordering::SeqCst), 3, "every write hit the sink");
    let failures = persist_failures(&mut receiver).await;
    assert_eq!(
        failures, 1,
        "exactly one UiSettingsPersistFailed is published per session"
    );
    let snapshot = writer.snapshot();
    assert_eq!(
        snapshot.ui_scale, 1.25,
        "memory keeps applying after failures"
    );
    drop(writer);
}

#[tokio::test(start_paused = true)]
async fn dropping_the_last_writer_ends_the_task() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let events = EventBus::default();
    {
        let writer = UiWriter::spawn(
            UiSettings::default(),
            recording_sink(Arc::clone(&count), Arc::clone(&calls)),
            events,
            &Handle::current(),
        );
        writer.mutate(UiKey::Renderer, |ui| {
            ui.renderer = crosshook_core::settings::UiRenderer::Gl;
        });
        settle(&calls, 1).await;
    }

    time::advance(Duration::from_secs(10)).await;
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    let after_drop = count.load(Ordering::SeqCst);
    time::advance(Duration::from_secs(10)).await;
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        count.load(Ordering::SeqCst),
        after_drop,
        "no sink calls may happen after the last writer drops"
    );
}

#[tokio::test(start_paused = true)]
async fn failed_write_keeps_keys_dirty_until_a_later_write_persists_them() {
    let calls: Arc<Mutex<Vec<Result<Record, ()>>>> = Arc::new(Mutex::new(Vec::new()));
    let fail = Arc::new(AtomicBool::new(true));
    let events = EventBus::default();
    let mut receiver = events.subscribe();
    let writer = UiWriter::spawn(
        UiSettings::default(),
        flaky_sink(Arc::clone(&fail), Arc::clone(&calls)),
        events,
        &Handle::current(),
    );

    writer.mutate(UiKey::Density, |ui| {
        ui.density = crosshook_core::settings::UiDensity::Spacious;
    });
    writer.flush().await.expect_err("first write fails");
    // The failed keys stay dirty: the background task is not notified and no
    // retry loop runs, but the very next flush retries them.
    writer
        .flush()
        .await
        .expect_err("retry of kept keys fails again");
    assert_eq!(
        calls.lock().unwrap().len(),
        2,
        "keys kept dirty are retried on the next flush"
    );

    fail.store(false, Ordering::SeqCst);
    writer.flush().await.expect("retried write succeeds");
    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 3, "the kept dirty key is persisted");
        assert!(
            matches!(&calls[2], Ok((_, patch)) if patch.keys.contains(UiKey::Density)),
            "the retried write carries the dirty key"
        );
    }
    writer.flush().await.expect("flush after success is quiet");
    assert_eq!(calls.lock().unwrap().len(), 3);
    assert_eq!(persist_failures(&mut receiver).await, 1);
    drop(writer);
}
