use super::*;

#[test]
fn claim_is_one_shot() {
    let flag = AtomicBool::new(false);
    assert!(claim(&flag));
    assert!(!claim(&flag));
}

#[test]
fn timespec_conversion_is_checked() {
    assert_eq!(timespec_to_ns(2, 5), Some(2_000_000_005));
    assert_eq!(timespec_to_ns(-1, 0), None);
    assert_eq!(timespec_to_ns(0, -1), None);
    assert_eq!(timespec_to_ns(i64::MAX, 0), None);
}

#[test]
fn monotonic_ns_is_nonzero_and_nondecreasing() {
    let first = monotonic_ns().unwrap();
    assert!(first > 0 && monotonic_ns().unwrap() >= first);
}

#[test]
fn write_line_to_emits_exactly_one_newline() {
    let mut out = Vec::new();
    write_line_to(&mut out, "READY 1").unwrap();
    assert_eq!(out, b"READY 1\n");
}

/// Counts `write` calls so a test can assert a marker line is one syscall-sized write.
struct CountingWriter {
    writes: Vec<Vec<u8>>,
}

impl std::io::Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writes.push(buf.to_vec());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn write_line_to_issues_the_line_and_newline_as_one_write() {
    let mut out = CountingWriter { writes: Vec::new() };
    write_line_to(&mut out, "BENCH_MARK 1 search 0 armed 5").unwrap();
    assert_eq!(
        out.writes,
        vec![b"BENCH_MARK 1 search 0 armed 5\n".to_vec()]
    );
}
