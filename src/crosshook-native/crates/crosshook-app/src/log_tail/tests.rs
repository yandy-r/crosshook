use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::rc::Rc;
use std::time::Duration;

use tokio::time::Instant;

use super::*;

fn append(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open for append");
    file.write_all(bytes).expect("append");
}

#[test]
fn fifty_thousand_lines_in_growth_steps_are_read_exactly_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("game.log");
    let content: String = (0..50_000).map(|i| format!("line {i} payload\n")).collect();
    let bytes = content.as_bytes();

    let mut tail = LogTail::new(&path);
    let mut got = Vec::new();
    // Uneven step boundaries land mid-line, exercising the partial buffer.
    let cuts = [0, 1, 7_777, 123_457, 300_001, 512_345, 700_003, bytes.len()];
    for step in cuts.windows(2) {
        append(&path, &bytes[step[0]..step[1]]);
        got.extend(tail.poll().expect("poll"));
    }
    got.extend(tail.poll().expect("final poll"));

    assert_eq!(got.len(), 50_000);
    for (i, line) in got.iter().enumerate() {
        assert_eq!(line, &format!("line {i} payload"));
    }
    assert_eq!(tail.bytes_read(), fs::metadata(&path).expect("meta").len());
    assert_eq!(tail.flush_partial(), None);
}

#[test]
fn missing_file_yields_nothing_then_resumes_when_created() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("late.log");
    let mut tail = LogTail::new(&path);
    assert!(tail.poll().expect("poll missing").is_empty());
    assert!(tail.poll().expect("poll missing again").is_empty());

    append(&path, b"hello\n");
    assert_eq!(tail.poll().expect("poll"), vec!["hello"]);
    assert_eq!(tail.bytes_read(), 6);
}

#[test]
fn truncation_restarts_from_zero_without_stale_lines() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("t.log");
    append(&path, b"one\ntwo\nthree\nstale-partial");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["one", "two", "three"]);

    // Truncate in place (same inode) and write shorter content.
    fs::write(&path, b"new\n").expect("truncate+write");
    assert_eq!(tail.poll().expect("poll after truncate"), vec!["new"]);
    assert!(tail.poll().expect("idle poll").is_empty());
    assert_eq!(tail.flush_partial(), None);
    assert_eq!(tail.bytes_read(), 27 + 4);
}

#[test]
fn rotation_drains_old_file_then_reads_new_file_from_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("r.log");
    let rotated = dir.path().join("r.log.1");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);

    // Writer appends to the old file, then it is rotated away unpolled.
    append(&path, b"b\nunterminated");
    fs::rename(&path, &rotated).expect("rename");
    append(&path, b"x\ny\n");

    assert_eq!(
        tail.poll().expect("poll after rotation"),
        vec!["b", "unterminated", "x", "y"]
    );
    assert!(tail.poll().expect("idle poll").is_empty());
    append(&path, b"z\n");
    assert_eq!(tail.poll().expect("poll new file"), vec!["z"]);
    // Late writes to the rotated file are not re-read.
    append(&rotated, b"late\n");
    assert!(tail.poll().expect("poll").is_empty());
    assert_eq!(tail.bytes_read(), 2 + 14 + 4 + 2);
}

#[test]
fn removed_file_is_drained_and_recreated_file_is_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("d.log");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);
    append(&path, b"b\n");
    fs::remove_file(&path).expect("remove");
    assert_eq!(tail.poll().expect("poll removed"), vec!["b"]);
    assert!(tail.poll().expect("poll missing").is_empty());
    append(&path, b"c\n");
    assert_eq!(tail.poll().expect("poll recreated"), vec!["c"]);
}

#[test]
fn partial_lines_join_crlf_is_stripped_and_invalid_utf8_is_replaced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("p.log");
    let mut tail = LogTail::new(&path);

    append(&path, b"hel");
    assert!(tail.poll().expect("poll").is_empty());
    append(&path, b"lo\r\nwor");
    assert_eq!(tail.poll().expect("poll"), vec!["hello"]);
    // A `\r` split from its `\n` across writes is still stripped.
    append(&path, b"ld\r");
    assert!(tail.poll().expect("poll").is_empty());
    append(&path, b"\nok\xff\xfe end\n");
    assert_eq!(
        tail.poll().expect("poll"),
        vec!["world".to_owned(), "ok\u{FFFD}\u{FFFD} end".to_owned()]
    );
    // Only a trailing `\r` is stripped; interior ones are content.
    append(&path, b"a\rb\n\n");
    assert_eq!(tail.poll().expect("poll"), vec!["a\rb", ""]);

    append(&path, b"tail\r");
    assert!(tail.poll().expect("poll").is_empty());
    assert_eq!(tail.flush_partial().as_deref(), Some("tail"));
    assert_eq!(tail.flush_partial(), None);
}

#[test]
fn utf8_char_split_across_appends_decodes_intact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("u.log");
    let mut tail = LogTail::new(&path);
    append(&path, b"caf\xc3");
    assert!(tail.poll().expect("poll").is_empty());
    append(&path, b"\xa9\n");
    assert_eq!(tail.poll().expect("poll"), vec!["café"]);
}

#[test]
fn unterminated_growth_is_emitted_in_capped_lines_on_char_boundaries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("n.log");
    let mut tail = LogTail::new(&path);

    // `é` (2 bytes) straddles the cap, so a byte cut would split it.
    let mut content = "a".repeat(PARTIAL_CAP - 1);
    content.push('é');
    content.push_str(&"b".repeat(PARTIAL_CAP));
    content.push_str("ü€");
    content.push_str(&"c".repeat(PARTIAL_CAP / 2));
    let bytes = content.as_bytes();

    let mut got = Vec::new();
    for piece in bytes.chunks(10_001) {
        append(&path, piece);
        got.extend(tail.poll().expect("poll"));
    }
    assert!(got.len() >= 2, "capped lines emitted before any newline");
    got.extend(tail.flush_partial());
    for line in &got {
        assert!(line.len() <= PARTIAL_CAP, "line of {} bytes", line.len());
        assert!(!line.contains('\u{FFFD}'), "split char decoded lossily");
    }
    assert_eq!(got.concat(), content);
    assert_eq!(tail.bytes_read(), bytes.len() as u64);
}

#[test]
fn large_backlog_is_read_in_bounded_chunks_across_polls() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("big.log");
    let content: String = (0..100_000)
        .map(|i| format!("backlog line {i:06}\n"))
        .collect();
    assert!(content.len() as u64 > 5 * READ_CHUNK);
    append(&path, content.as_bytes());

    let mut tail = LogTail::new(&path);
    let mut got = Vec::new();
    let mut polls = 0;
    loop {
        let before = tail.bytes_read();
        got.extend(tail.poll().expect("poll"));
        polls += 1;
        assert!(tail.bytes_read() - before <= READ_CHUNK);
        if !tail.has_backlog() {
            break;
        }
    }
    assert!(polls > 5, "backlog spread over polls, got {polls}");
    assert!(tail.poll().expect("idle poll").is_empty());
    assert_eq!(got.len(), 100_000);
    for (i, line) in got.iter().enumerate() {
        assert_eq!(line, &format!("backlog line {i:06}"));
    }
    assert_eq!(tail.bytes_read(), fs::metadata(&path).expect("meta").len());
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
            assert!(!tl.final_seen, "poll after final poll");
            tl.final_seen = mode != PollMode::Live;
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
            Ok((tl, lines, false))
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
        async move { Ok((n + 1, vec![format!("chunk {n}")], true)) }
    };
    let mut batches = 0;
    tokio::time::timeout(
        Duration::from_secs(1),
        drive(0_u32, poll, TailConfig::default(), cancel, |_| batches += 1),
    )
    .await
    .expect("cancel returns promptly")
    .expect("drive");

    let modes = modes.borrow();
    let bound = FINAL_DRAIN_CHUNKS as usize;
    assert_eq!(modes.len(), bound);
    assert!(modes[..bound - 1].iter().all(|m| *m == PollMode::Drain));
    assert_eq!(modes[bound - 1], PollMode::Last);
    assert_eq!(batches, bound);
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
