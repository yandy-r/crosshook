//! Stream tee, lag and ordering tests (Task 2.3).
//!
//! Covers the lossy pipe tee (D7), the `follow`-driven tail replacing the
//! 500 ms re-read (D3), the bounded-channel lag mapping (D2) and the
//! no-kill rule.

use std::process::Stdio;
use std::time::{Duration, Instant};

use crosshook_core::launch::{SessionId, SessionKind};
use tempfile::TempDir;
use tokio::process::Command;

use super::super::streaming::{stream_child_log, stream_session};
use super::super::types::{EventPublisher, LAUNCH_EVENT_CAPACITY};
use super::super::LaunchEvent;
use crate::log_tail::{LogBatch, PARTIAL_CAP};

/// All lines from `LogBatch::Lines` events of `session_id`, in order.
fn log_lines(events: &[LaunchEvent], session_id: SessionId) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            LaunchEvent::Log {
                session_id: id,
                batch: LogBatch::Lines(lines),
            } if *id == session_id => Some(lines.iter().cloned()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Spawns a piped `sh -c body` child wired like a Proton game launch.
fn piped_child(body: &str) -> tokio::process::Child {
    let mut child = Command::new("sh");
    child
        .arg("-c")
        .arg(body)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    child.spawn().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lossy_pipe_tee_survives_bad_utf8_and_overlong_line() {
    let root = TempDir::new().unwrap();
    let log_path = root.path().join("h2.log");
    std::fs::File::create(&log_path).unwrap();

    // \377\376 (invalid UTF-8) line, then 1 MiB with no newline, then exit 0
    // (H-2). Octal escapes: POSIX printf (dash on CI) has no `\x`.
    let mut child = piped_child(
        "printf 'bad\\377\\376\\n'; head -c 1048576 /dev/zero | tr '\\0' 'A'; printf '\\n'",
    );
    let status = stream_child_log(&mut child, &log_path, |_| {})
        .await
        .unwrap();
    assert!(status.success(), "exit 0 despite bad bytes and a huge line");

    // Every byte is in the log, byte-exact: over-long reads pass through
    // without inserted separators; decoded lines are lossy and bounded (D7).
    let log = std::fs::read(&log_path).unwrap();
    assert_eq!(
        log.len(),
        6 + 1_048_576 + 1,
        "every tee byte landed in the log, with no synthetic separators"
    );
    assert_eq!(log[6..].iter().filter(|b| **b == b'\n').count(), 1);
    assert!(log.starts_with(b"bad\xff\xfe\n"));
    let text = String::from_utf8_lossy(&log);
    assert!(
        text.contains("bad\u{FFFD}\u{FFFD}"),
        "invalid UTF-8 decoded lossily"
    );
    // The log itself is byte-exact, so it still holds the full 1 MiB
    // newline-free run; the 64 KiB cap applies to the *decoded tail lines*
    // (`LogTail`), not to the stored bytes.
    assert_eq!(text.lines().count(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn newline_free_flood_is_streamed_with_bounded_memory() {
    let root = TempDir::new().unwrap();
    let log_path = root.path().join("flood.log");
    std::fs::File::create(&log_path).unwrap();
    let registry = crosshook_core::launch::LaunchSessionRegistry::new();
    let (session_id, _rx) = registry.register(SessionKind::Game, "newline-free");
    let (publisher, mut events) = EventPublisher::channel(session_id);

    // 8 MiB with no newline, then sleep: the tail must already be flowing
    // (bounded reads, LogTail's partial cap) long before the child exits.
    let mut child = piped_child("head -c 8388608 /dev/zero | tr '\\0' 'A'; sleep 2; printf '\\n'");
    let publisher_clone = publisher.clone();
    let log_for_stream = log_path.clone();
    let stream = tokio::spawn(async move {
        stream_child_log(&mut child, &log_for_stream, move |batch| {
            publisher_clone.log(batch);
        })
        .await
    });

    // Wait until the writer has streamed well past one PARTIAL_CAP piece,
    // while the child is still sleeping.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let written = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
        if written > (PARTIAL_CAP * 4) as u64 {
            break;
        }
        assert!(Instant::now() < deadline, "log never grew; pipe blocked");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // The sink is already receiving capped partial lines, even though the
    // child has not written a newline or exited.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut streamed_bytes = 0usize;
    while Instant::now() < deadline {
        match events.recv().await {
            Some(LaunchEvent::Log {
                batch: LogBatch::Lines(lines),
                ..
            }) => streamed_bytes += lines.iter().map(|l| l.len() + 1).sum::<usize>(),
            Some(_) => {}
            None => break,
        }
        if streamed_bytes > PARTIAL_CAP * 2 {
            break;
        }
    }
    assert!(
        streamed_bytes > PARTIAL_CAP * 2,
        "partial lines streamed live, got {streamed_bytes} bytes"
    );
    assert!(!stream.is_finished(), "child is still sleeping");

    let status = stream.await.unwrap().unwrap();
    assert!(status.success(), "child exited on its own");

    // Byte-exact: 8 MiB of 'A' plus the trailing newline, nothing inserted.
    let log = std::fs::read(&log_path).unwrap();
    assert_eq!(log.len(), 8_388_608 + 1, "log is byte-exact");
    assert!(log.iter().take(8_388_608).all(|b| *b == b'A'));
    assert_eq!(log[8_388_608], b'\n');
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exiting_after_a_large_burst_delivers_every_line_or_a_truncated_notice() {
    let root = TempDir::new().unwrap();
    let log_path = root.path().join("burst.log");
    std::fs::File::create(&log_path).unwrap();
    let registry = crosshook_core::launch::LaunchSessionRegistry::new();
    let (session_id, _rx) = registry.register(SessionKind::Game, "burst-exit");
    let (publisher, mut events) = EventPublisher::channel(session_id);

    // ~2.4 MiB of distinct numbered lines written in one burst, then exit.
    let mut child = piped_child("seq 1 360000");
    let publisher_clone = publisher.clone();
    let status = stream_child_log(&mut child, &log_path, move |batch| {
        publisher_clone.log(batch);
    })
    .await
    .unwrap();
    assert!(status.success());
    // Close the channel so the drain terminates once the queue is empty.
    drop(publisher);

    let mut saw: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut truncated_bytes = None;
    for event in events.drain_ready() {
        if let LaunchEvent::Log { batch, .. } = event {
            match batch {
                LogBatch::Lines(lines) => saw.extend(lines),
                LogBatch::Truncated { bytes } => truncated_bytes = Some(bytes),
                // Broadcast lag counts events, a different unit; the burst
                // may legitimately lag the unbounded-by-test receiver.
                LogBatch::Dropped { .. } => {}
            }
        }
    }
    match truncated_bytes {
        None => {
            // Unless the receiver lagged, every line arrived.
            if saw.len() == 360_000 {
                for n in [1u32, 180_000, 360_000] {
                    assert!(saw.contains(&n.to_string()), "line {n} delivered");
                }
            }
        }
        Some(bytes) => {
            // Estimated unread *bytes*, bounded by the whole log file.
            let log_len = std::fs::metadata(&log_path).unwrap().len();
            assert!(bytes > 0 && bytes <= log_len, "{bytes} of {log_len} bytes");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_log_follow_still_returns_the_real_exit_status() {
    let root = TempDir::new().unwrap();
    // A directory opens fine but every read fails (EISDIR), so the follow
    // ends with an error once the child has exited.
    let log_path = root.path().join("not-a-file.log");
    std::fs::create_dir(&log_path).unwrap();
    let mut child = Command::new("sh");
    child
        .arg("-c")
        .arg("sleep 0.2; exit 7")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = child.spawn().unwrap();

    let status = stream_child_log(&mut child, &log_path, |_| {})
        .await
        .expect("a follow failure is logged, not returned");
    assert_eq!(status.code(), Some(7));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdout_and_stderr_both_stream_and_final_lines_arrive_before_return() {
    let root = TempDir::new().unwrap();
    let log_path = root.path().join("both.log");
    std::fs::File::create(&log_path).unwrap();
    let registry = crosshook_core::launch::LaunchSessionRegistry::new();
    let (session_id, _rx) = registry.register(SessionKind::Game, "stream-both");
    let (publisher, mut events) = EventPublisher::channel(session_id);

    let mut child =
        piped_child("echo out-one; echo err-one >&2; sleep 0.1; echo out-two; echo err-two >&2");
    let publisher_clone = publisher.clone();
    let status = stream_child_log(&mut child, &log_path, move |batch| {
        publisher_clone.log(batch);
    })
    .await
    .unwrap();
    assert!(status.success());

    // Final drain: every batch was delivered before stream_child_log returned.
    let lines = log_lines(&events.drain_ready(), session_id);
    for expected in ["out-one", "err-one", "out-two", "err-two"] {
        assert!(
            lines.iter().any(|line| line == expected),
            "missing {expected:?} in {lines:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_mode_child_streams_without_a_tee() {
    let root = TempDir::new().unwrap();
    let log_path = root.path().join("file-mode.log");
    std::fs::File::create(&log_path).unwrap();
    let registry = crosshook_core::launch::LaunchSessionRegistry::new();
    let (session_id, _rx) = registry.register(SessionKind::Game, "file-mode");
    let (publisher, mut events) = EventPublisher::channel(session_id);

    // File-mode child: stdout/stderr go to the log itself, like the Steam
    // helper. There is no pipe to tee; follow must still stream.
    let file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .unwrap();
    let file_err = file.try_clone().unwrap();
    let mut child = Command::new("sh");
    child
        .arg("-c")
        .arg("echo file-line-one; sleep 0.1; echo file-line-two")
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(file_err));
    let mut child = child.spawn().unwrap();

    let publisher_clone = publisher.clone();
    let status = stream_child_log(&mut child, &log_path, move |batch| {
        publisher_clone.log(batch);
    })
    .await
    .unwrap();
    assert!(status.success());

    let lines = log_lines(&events.drain_ready(), session_id);
    assert!(
        lines.iter().any(|line| line == "file-line-one"),
        "first line streamed: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line == "file-line-two"),
        "final line delivered by the bounded drain: {lines:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lagging_receiver_sees_dropped_notice_and_exited_last() {
    let root = TempDir::new().unwrap();
    let log_path = root.path().join("lag.log");
    std::fs::File::create(&log_path).unwrap();
    let registry = crosshook_core::launch::LaunchSessionRegistry::new();
    let (session_id, _rx) = registry.register(SessionKind::Game, "lag-test");
    let (publisher, mut events) = EventPublisher::channel(session_id);

    // A streaming child runs while the sink publishes; a flood of batches —
    // as a burst of child output produces through the same publisher — then
    // overruns the bounded session channel while the receiver never reads.
    let mut child = piped_child("sleep 0.3; echo done");
    let publisher_clone = publisher.clone();
    let stream = tokio::spawn(async move {
        stream_child_log(&mut child, &log_path, move |batch| {
            publisher_clone.log(batch);
        })
        .await
    });
    for n in 0..(LAUNCH_EVENT_CAPACITY + 64) {
        publisher.log(LogBatch::Lines(vec![format!("flood-{n}")]));
    }
    let status = stream.await.unwrap().unwrap();
    assert!(status.success());

    // The receiver never read while streaming: it must now see a Dropped
    // notice for the skipped batches, and the terminal Exited lands last.
    publisher.exited(status.code(), None);
    let collect = async {
        let mut saw_dropped = false;
        let mut last = None;
        while let Some(event) = events.recv().await {
            saw_dropped |= matches!(
                event,
                LaunchEvent::Log {
                    batch: LogBatch::Dropped { n },
                    ..
                } if n > 0
            );
            let exited = matches!(event, LaunchEvent::Exited { .. });
            last = Some(event);
            if exited {
                break;
            }
        }
        (saw_dropped, last)
    };
    let (saw_dropped, last) = tokio::time::timeout(Duration::from_secs(5), collect)
        .await
        .expect("the queued events drain within 5 s");
    assert!(
        saw_dropped,
        "a Dropped notice reached the lagging receiver (capacity {LAUNCH_EVENT_CAPACITY})"
    );
    assert!(
        matches!(last, Some(LaunchEvent::Exited { .. })),
        "Exited is delivered last"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_session_drains_forever_without_killing_the_child() {
    let root = TempDir::new().unwrap();
    let registry = crosshook_core::launch::LaunchSessionRegistry::new();
    let (session_id, _rx) = registry.register(SessionKind::Game, "never-kill");
    let (publisher, _events) = EventPublisher::channel(session_id);

    // The log path cannot be opened (missing parent): the tee keeps draining
    // the child's pipes to EOF instead of killing it, so `yes` — which would
    // block on a full pipe within milliseconds if undrained — finishes on
    // its own with exit 0.
    let missing = root.path().join("no-such-dir").join("missing.log");
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg("yes | head -c 1048576")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(false);
    let child = command.spawn().unwrap();

    let started = Instant::now();
    let status = stream_session(child, &missing, &publisher).await;
    assert_eq!(
        status.map(|status| status.success()),
        Some(true),
        "child drained to EOF and exited on its own"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "an undrained pipe would have stalled the child"
    );
}
