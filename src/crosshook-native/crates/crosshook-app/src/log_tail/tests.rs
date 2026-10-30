use std::fs::{self, OpenOptions};
use std::io::{Seek, Write};
use std::time::Duration;

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
        // The per-poll line budget may spread one step over several polls.
        loop {
            got.extend(tail.poll().expect("poll"));
            if !tail.has_backlog() {
                break;
            }
        }
    }

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
fn a_replaced_file_is_drained_then_the_log_ends() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("r.log");
    let rotated = dir.path().join("r.log.1");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);

    // Writer appends to the open file, then another file takes its name.
    append(&path, b"b\nunterminated");
    fs::rename(&path, &rotated).expect("rename");
    append(&path, b"swapped-in\n");

    assert_eq!(
        tail.poll().expect("poll after replacement"),
        vec!["b", "unterminated"]
    );
    append(&path, b"more\n");
    append(&rotated, b"late\n");
    assert!(tail.poll().expect("ended").is_empty());
    assert_eq!(tail.bytes_read(), 2 + 14);
}

#[test]
fn a_removed_file_is_drained_and_a_recreated_one_is_never_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("d.log");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);
    append(&path, b"b\n");
    fs::remove_file(&path).expect("remove");
    assert_eq!(tail.poll().expect("poll removed"), vec!["b"]);
    append(&path, b"c\n");
    assert!(tail.poll().expect("ended").is_empty());
}

#[test]
fn a_symlink_swapped_in_for_the_log_is_never_followed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("s.log");
    let secret = dir.path().join("secret.txt");
    fs::write(&secret, b"secret contents\n").expect("secret");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);

    fs::remove_file(&path).expect("remove");
    std::os::unix::fs::symlink(&secret, &path).expect("symlink");

    assert!(tail.poll().expect("drain").is_empty());
    assert!(tail.poll().expect("ended").is_empty());
    assert_eq!(tail.bytes_read(), 2, "the symlink target is never read");
}

/// Runs `poll` on a helper thread and fails instead of hanging the test
/// binary if it blocks (an open of a FIFO without `O_NONBLOCK` would).
fn poll_on_thread(mut tail: LogTail) -> (LogTail, io::Result<Vec<String>>) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = tail.poll();
        let _ = tx.send((tail, result));
    });
    rx.recv_timeout(Duration::from_secs(10))
        .expect("poll must not block on a FIFO")
}

fn make_fifo(path: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo");
    assert!(status.success());
}

#[test]
fn a_fifo_swapped_in_for_the_log_is_never_opened() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("f.log");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);
    fs::remove_file(&path).expect("remove");
    make_fifo(&path);

    let (tail, drained) = poll_on_thread(tail);
    assert!(drained.expect("drain").is_empty());
    let (_, ended) = poll_on_thread(tail);
    assert!(ended.expect("ended").is_empty());
}

#[test]
fn an_unpinned_tail_refuses_a_symlink_or_fifo_at_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let secret = dir.path().join("secret.txt");
    fs::write(&secret, b"secret contents\n").expect("secret");
    let link = dir.path().join("link.log");
    std::os::unix::fs::symlink(&secret, &link).expect("symlink");
    let fifo = dir.path().join("fifo.log");
    make_fifo(&fifo);

    let (_, linked) = poll_on_thread(LogTail::new(&link));
    assert!(linked.is_err(), "a symlink is refused: {linked:?}");
    let (_, piped) = poll_on_thread(LogTail::new(&fifo));
    assert!(piped.is_err(), "a FIFO is refused: {piped:?}");
}

#[test]
fn a_pinned_tail_reads_only_the_created_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("p.log");
    let moved = dir.path().join("p.log.moved");
    let created = File::create(&path).expect("create");
    // Swapped before the first poll: the tail must never read the impostor.
    fs::rename(&path, &moved).expect("rename");
    append(&path, b"impostor\n");
    let mut tail = LogTail::pinned(&path, &created).expect("pinned");
    assert!(tail.poll().expect("mismatch ends the log").is_empty());
    fs::remove_file(&path).expect("remove impostor");
    fs::rename(&moved, &path).expect("restore");
    append(&path, b"real\n");
    assert!(tail.poll().expect("ended").is_empty());
    assert_eq!(tail.bytes_read(), 0);

    // Unswapped, the pinned tail reads the created file normally.
    let path = dir.path().join("q.log");
    let created = File::create(&path).expect("create");
    let mut tail = LogTail::pinned(&path, &created).expect("pinned");
    append(&path, b"one\n");
    assert_eq!(tail.poll().expect("poll"), vec!["one"]);
}

#[test]
fn a_pinned_tail_ends_the_log_when_the_path_is_a_symlink_to_the_created_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let real = dir.path().join("real.log");
    let path = dir.path().join("p.log");
    let created = File::create(&real).expect("create");
    std::os::unix::fs::symlink(&real, &path).expect("symlink");
    append(&real, b"line\n");

    let (tail, result) = poll_on_thread(LogTail::pinned(&path, &created).expect("pinned"));
    assert!(result.expect("refused open ends the log").is_empty());
    assert_eq!(tail.bytes_read(), 0);
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
fn unread_estimate_counts_only_the_open_file_after_it_is_replaced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("r.log");
    let rotated = dir.path().join("r.log.1");
    append(&path, b"a\n");
    let mut tail = LogTail::new(&path);
    assert_eq!(tail.poll().expect("poll"), vec!["a"]);
    append(&path, b"b\n");
    fs::rename(&path, &rotated).expect("rename");
    append(&path, b"x\ny\nzzzz\n");

    // 2 unread bytes on the old handle; the file now at the path is never
    // read, so it is not counted.
    assert_eq!(tail.unread_bytes(), 2);
}

/// Reader serving at most `remaining` bytes from the file, then failing.
pub(super) struct FailAfter<'a> {
    file: &'a mut File,
    remaining: u64,
}

impl<'a> FailAfter<'a> {
    pub(super) fn new(file: &'a mut File, remaining: u64) -> Self {
        Self { file, remaining }
    }
}

impl Read for FailAfter<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("simulated read failure"));
        }
        let cap = buf
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let n = self.file.read(&mut buf[..cap])?;
        self.remaining -= n as u64;
        Ok(n)
    }
}

fn assert_position_matches_offset(tail: &mut LogTail) {
    let open = tail.file.as_mut().expect("open file");
    assert_eq!(open.file.stream_position().expect("position"), open.offset);
}

#[test]
fn a_mid_read_failure_keeps_the_bytes_read_without_loss_or_duplication() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("m.log");
    append(&path, b"a\nb\nc\nd\n");
    let mut tail = LogTail::new(&path);
    tail.fault = Some(5); // serves "a\nb\nc", then fails

    assert_eq!(tail.poll().expect("partial read is kept"), vec!["a", "b"]);
    assert_eq!(tail.bytes_read(), 5);
    assert_position_matches_offset(&mut tail);
    // The rest of the file arrives next, completing the buffered "c".
    assert_eq!(tail.poll().expect("retry"), vec!["c", "d"]);
    assert_eq!(tail.bytes_read(), 8);
    assert_position_matches_offset(&mut tail);
    assert_eq!(tail.flush_partial(), None);
}

#[test]
fn a_read_failing_before_any_byte_leaves_the_tail_untouched_and_retryable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("z.log");
    append(&path, b"a\nb\n");
    let mut tail = LogTail::new(&path);
    tail.fault = Some(0);

    assert!(tail.poll().is_err());
    assert_eq!(tail.bytes_read(), 0);
    assert_position_matches_offset(&mut tail);
    assert_eq!(tail.poll().expect("retry"), vec!["a", "b"]);
    assert_position_matches_offset(&mut tail);
}

#[test]
fn a_flood_of_one_byte_lines_is_delivered_in_bounded_batches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tiny.log");
    let total = MAX_LINES_PER_POLL * 2 + 123;
    let content = "a\n".repeat(total);
    append(&path, content.as_bytes());

    let mut tail = LogTail::new(&path);
    let mut delivered = 0;
    let mut batches = Vec::new();
    loop {
        let lines = tail.poll().expect("poll");
        assert!(
            lines.len() <= MAX_LINES_PER_POLL,
            "batch of {}",
            lines.len()
        );
        delivered += lines.len();
        batches.push(lines.len());
        if !tail.has_backlog() {
            break;
        }
    }
    assert_eq!(delivered, total, "nothing lost: {batches:?}");
    assert_eq!(batches.len(), 3, "{batches:?}");
    assert_eq!(tail.bytes_read(), content.len() as u64);
    assert_eq!(tail.flush_partial(), None);
}
