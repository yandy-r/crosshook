use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use tokio::time::Instant;

use super::*;
use crate::log_tail::MAX_LINES_PER_POLL;

fn append(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open for append");
    file.write_all(bytes).expect("append");
}

#[test]
fn config_clamps_window_and_backoff() {
    let (w, max) = TailConfig {
        coalesce_window: Duration::from_millis(1),
        idle_backoff_max: Duration::from_millis(5),
    }
    .effective();
    assert_eq!(w, Duration::from_millis(16));
    assert_eq!(max, Duration::from_millis(16));

    let (w, max) = TailConfig {
        coalesce_window: Duration::from_secs(1),
        idle_backoff_max: Duration::from_secs(2),
    }
    .effective();
    assert_eq!(w, Duration::from_millis(33));
    assert_eq!(max, Duration::from_secs(2));

    let default = TailConfig::default();
    assert_eq!(default.coalesce_window, Duration::from_millis(33));
    assert_eq!(default.idle_backoff_max, Duration::from_millis(500));
}

/// Simulated file: lines become visible at their write time (ms from start).
#[derive(Debug)]
struct Timeline {
    start: Instant,
    writes: Vec<(u64, String)>,
    next: usize,
    final_seen: bool,
}

#[derive(Default)]
struct Record {
    polls: Vec<u64>,
    batches: Vec<(u64, LogBatch)>,
}

fn ms_since(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).expect("ms fits")
}

async fn run_timeline(writes: Vec<(u64, String)>, cancel_at_ms: u64) -> Record {
    let start = Instant::now();
    let record = Rc::new(RefCell::new(Record::default()));
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(cancel_at_ms)).await;
        canceller.cancel();
    });

    let poll_record = Rc::clone(&record);
    let poll = move |mut tl: Timeline, mode: PollMode| {
        let rec = Rc::clone(&poll_record);
        async move {
            assert!(
                mode != PollMode::Live || !tl.final_seen,
                "live poll after final poll"
            );
            if mode != PollMode::Live {
                tl.final_seen = true;
            }
            let now = ms_since(tl.start);
            rec.borrow_mut().polls.push(now);
            let mut lines = Vec::new();
            while let Some((at, line)) = tl.writes.get(tl.next) {
                if *at > now {
                    break;
                }
                lines.push(line.clone());
                tl.next += 1;
            }
            Ok((tl, lines, false, None))
        }
    };
    let sink_record = Rc::clone(&record);
    let sink = move |batch| {
        let now = ms_since(start);
        sink_record.borrow_mut().batches.push((now, batch));
    };
    let timeline = Timeline {
        start,
        writes,
        next: 0,
        final_seen: false,
    };
    drive(timeline, poll, TailConfig::default(), cancel, sink)
        .await
        .expect("drive");
    Rc::try_unwrap(record)
        .ok()
        .expect("sole owner")
        .into_inner()
}

fn lines(range: std::ops::Range<u32>, prefix: &str) -> Vec<String> {
    range.map(|i| format!("{prefix}{i}")).collect()
}

#[tokio::test(start_paused = true)]
async fn coalesces_per_window_and_backs_off_when_idle() {
    let mut writes: Vec<(u64, String)> = Vec::new();
    // Burst at t=0 → first batch; burst inside (0, 33] → second batch.
    writes.extend(lines(0..10, "a").into_iter().map(|l| (0, l)));
    writes.extend(
        lines(0..500, "b")
            .into_iter()
            .enumerate()
            .map(|(i, l)| (1 + (i as u64 % 30), l)),
    );
    // Separate window → separate batch.
    writes.push((40, "c".to_owned()));
    // After a long idle stretch, one more line.
    writes.push((1_600, "d".to_owned()));

    let rec = run_timeline(writes, 2_100).await;

    // Data flowing: every window. Idle: 66, 132, 264, 500 (cap), 500.
    // Data at 2061 resets the interval to the 33 ms window; cancel at 2100
    // triggers the final poll.
    assert_eq!(
        rec.polls,
        vec![0, 33, 66, 99, 165, 297, 561, 1_061, 1_561, 2_061, 2_094, 2_100]
    );
    assert_eq!(
        rec.batches,
        vec![
            (0, LogBatch::Lines(lines(0..10, "a"))),
            (33, LogBatch::Lines(lines(0..500, "b"))),
            (66, LogBatch::Lines(vec!["c".to_owned()])),
            (2_061, LogBatch::Lines(vec!["d".to_owned()])),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn cancel_runs_final_poll_and_never_emits_empty_batches() {
    let writes = vec![(10, "late".to_owned())];
    let rec = run_timeline(writes, 50).await;
    // 0 (empty → 66), cancel at 50 interrupts the sleep for the final poll.
    assert_eq!(rec.polls, vec![0, 50]);
    assert_eq!(
        rec.batches,
        vec![(50, LogBatch::Lines(vec!["late".to_owned()]))]
    );

    let rec = run_timeline(Vec::new(), 50).await;
    assert_eq!(rec.polls, vec![0, 50]);
    assert!(rec.batches.is_empty());
}

#[tokio::test(start_paused = true)]
async fn cancel_drain_is_bounded_when_backlog_never_ends() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let modes = Rc::new(RefCell::new(Vec::new()));
    let poll_modes = Rc::clone(&modes);
    let poll = move |n: u32, mode: PollMode| {
        poll_modes.borrow_mut().push(mode);
        async move {
            let truncated = (mode == PollMode::Last).then_some(9);
            Ok((n + 1, vec![format!("chunk {n}")], true, truncated))
        }
    };
    let mut batches = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(1),
        drive(0_u32, poll, TailConfig::default(), cancel, |b| {
            batches.push(b);
        }),
    )
    .await
    .expect("cancel returns promptly")
    .expect("drive");

    let modes = modes.borrow();
    let bound = FINAL_DRAIN_CHUNKS as usize;
    assert_eq!(modes.len(), bound);
    assert!(modes[..bound - 1].iter().all(|m| *m == PollMode::Drain));
    assert_eq!(modes[bound - 1], PollMode::Last);
    // One lines batch per drain poll, then the cut-short notice in bytes.
    assert_eq!(batches.len(), bound + 1);
    assert_eq!(batches.last(), Some(&LogBatch::Truncated { bytes: 9 }));
}

#[tokio::test(start_paused = true)]
async fn live_backlog_polls_again_without_waiting_for_a_window() {
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    let start = Instant::now();
    let record = Rc::new(RefCell::new(Vec::new()));
    let rec = Rc::clone(&record);
    let poll = move |n: u32, mode: PollMode| {
        if n == 3 {
            canceller.cancel();
        }
        rec.borrow_mut().push((n, ms_since(start), mode));
        // Backlog pending throughout the live phase; drained once cancelled.
        async move {
            let more = mode == PollMode::Live;
            Ok((n + 1, vec![format!("l{n}")], more, None))
        }
    };
    let mut batches = Vec::new();
    drive(0_u32, poll, TailConfig::default(), cancel, |b| {
        batches.push(b);
    })
    .await
    .expect("drive");
    let record = record.borrow();
    // Four live polls and one drain poll, all at t=0.
    assert_eq!(record.len(), 5, "polls: {record:?} batches: {batches:?}");
    assert!(
        record.iter().all(|(_, at, _)| *at == 0),
        "backlog polls never slept: {record:?}"
    );
    assert!(
        batches.iter().all(|b| matches!(b, LogBatch::Lines(_))),
        "only line batches, no Dropped: {batches:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn transient_poll_errors_retry_with_backoff_and_recover() {
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    let start = Instant::now();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let call_log = Rc::clone(&calls);
    let poll = move |n: u32, _mode: PollMode| {
        call_log.borrow_mut().push(ms_since(start));
        let canceller = canceller.clone();
        async move {
            match n {
                1 | 2 => Err((Some(n + 1), io::Error::other("simulated poll failure"))),
                3 => {
                    canceller.cancel();
                    Ok((4, vec!["recovered".to_owned()], false, None))
                }
                _ => Ok((n + 1, Vec::new(), false, None)),
            }
        }
    };
    let mut batches = Vec::new();
    drive(0_u32, poll, TailConfig::default(), cancel, |b| {
        batches.push(b);
    })
    .await
    .expect("drive recovers from transient errors");

    let calls = calls.borrow();
    // Poll 1 (n=0) succeeds but is empty, so the interval backs off to 66;
    // each subsequent error doubles it again (132, 264). After recovery the
    // cancellation ends the run immediately (biased select).
    assert_eq!(*calls, vec![0, 66, 198, 462, 462], "poll times: {calls:?}");
    assert_eq!(batches, vec![LogBatch::Lines(vec!["recovered".to_owned()])]);
}

#[tokio::test(start_paused = true)]
async fn persistent_poll_errors_stop_after_a_bounded_streak() {
    let cancel = CancellationToken::new();
    let calls = Rc::new(RefCell::new(0_u32));
    let call_count = Rc::clone(&calls);
    let poll = move |n: u32, _mode: PollMode| {
        *call_count.borrow_mut() += 1;
        async move {
            Err::<(u32, Vec<String>, bool, Option<u64>), _>((Some(n), io::Error::other("down")))
        }
    };
    let result = drive(0_u32, poll, TailConfig::default(), cancel, |_| {}).await;
    assert!(result.is_err(), "a persistent failure surfaces");
    assert_eq!(
        *calls.borrow(),
        MAX_CONSECUTIVE_POLL_ERRORS,
        "one attempt per error, then give up"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn follow_emits_truncated_notice_when_the_final_drain_is_capped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("flood.log");
    // More short lines than the capped drain can deliver
    // (FINAL_DRAIN_CHUNKS × MAX_LINES_PER_POLL).
    let total = FINAL_DRAIN_CHUNKS as usize * MAX_LINES_PER_POLL + 50_000;
    let content: String = (0..total).map(|i| format!("flood {i:06}\n")).collect();
    append(&path, content.as_bytes());

    let cancel = CancellationToken::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(follow(
        path.clone(),
        TailConfig::default(),
        cancel.clone(),
        move |batch| {
            tx.send(batch).expect("receiver alive");
        },
    ));
    // Cancel before the live phase can read (test file written above).
    cancel.cancel();
    tokio::time::timeout(FOLLOW_WAIT, task)
        .await
        .expect("follow stops in time")
        .expect("join")
        .expect("follow");

    let mut total_lines = 0usize;
    let mut truncated = None;
    while let Ok(Some(batch)) = recv(&mut rx).await {
        match batch {
            LogBatch::Lines(lines) => total_lines += lines.len(),
            LogBatch::Truncated { bytes } => truncated = Some(bytes),
            LogBatch::Dropped { n } => panic!("lag notice from a file tail: {n}"),
        }
    }
    // Either everything was drained live, or the capped drain left a visible
    // notice — no silent loss either way.
    if total_lines < total {
        let bytes = truncated.expect("a Truncated notice covers the unread remainder");
        let line_len = "flood 000000\n".len();
        // Bytes of the undelivered lines, never an event count.
        assert_eq!(bytes, ((total - total_lines) * line_len) as u64);
    }
}

#[tokio::test(start_paused = true)]
async fn final_drain_reports_truncation_only_for_a_nonzero_estimate() {
    for (estimate, expected) in [
        (Some(0), None),
        (None, None),
        (Some(7), Some(LogBatch::Truncated { bytes: 7 })),
    ] {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let mut batches = Vec::new();
        let poll = move |n: u32, mode: PollMode| async move {
            let est = if mode == PollMode::Last {
                estimate
            } else {
                None
            };
            Ok((n, Vec::new(), true, est))
        };
        drive(0_u32, poll, TailConfig::default(), cancel, |b| {
            batches.push(b);
        })
        .await
        .expect("drive");
        assert_eq!(batches, expected.into_iter().collect::<Vec<_>>());
    }
}

#[tokio::test]
async fn a_panicking_blocking_poll_is_a_terminal_error_not_an_unwind() {
    fn panics(_: &mut LogTail, _: PollMode) -> io::Result<(Vec<String>, bool, Option<u64>)> {
        panic!("simulated blocking read panic");
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let tail = LogTail::new(dir.path().join("p.log"));
    let Err((state, error)) = blocking_poll(tail, PollMode::Live, panics).await else {
        panic!("a panicked poll must not succeed");
    };
    assert!(state.is_none(), "the tail is gone with the panicked task");
    assert!(error.to_string().contains("blocking read task failed"));

    let cancel = CancellationToken::new();
    let poll = |tail: LogTail, mode: PollMode| blocking_poll(tail, mode, panics);
    let tail = LogTail::new(dir.path().join("p.log"));
    let result = drive(tail, poll, TailConfig::default(), cancel, |_| {}).await;
    assert!(
        result.is_err(),
        "drive surfaces it at once instead of retrying"
    );
}

const FOLLOW_WAIT: Duration = Duration::from_secs(5);

async fn recv(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<LogBatch>,
) -> Result<Option<LogBatch>, tokio::time::error::Elapsed> {
    tokio::time::timeout(FOLLOW_WAIT, rx.recv()).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn follow_streams_file_and_flushes_partial_on_cancel() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("f.log");
    append(&path, b"first\nsecond\n");

    let cancel = CancellationToken::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(follow(
        path.clone(),
        TailConfig::default(),
        cancel.clone(),
        move |batch| {
            tx.send(batch).expect("receiver alive");
        },
    ));

    assert_eq!(
        recv(&mut rx).await.expect("first batch in time"),
        Some(LogBatch::Lines(vec![
            "first".to_owned(),
            "second".to_owned()
        ]))
    );
    append(&path, b"third\nunterminated");
    assert_eq!(
        recv(&mut rx).await.expect("second batch in time"),
        Some(LogBatch::Lines(vec!["third".to_owned()]))
    );

    cancel.cancel();
    tokio::time::timeout(FOLLOW_WAIT, task)
        .await
        .expect("follow stops in time")
        .expect("join")
        .expect("follow");
    assert_eq!(
        recv(&mut rx).await.expect("flush batch in time"),
        Some(LogBatch::Lines(vec!["unterminated".to_owned()]))
    );
    assert_eq!(recv(&mut rx).await.expect("close in time"), None);
}
