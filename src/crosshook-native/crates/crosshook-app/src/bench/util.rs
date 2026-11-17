//! Helpers shared by the READY and marker writers: the monotonic clock, the
//! one-shot claim and the single-line stderr write.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use nix::time::{clock_gettime, ClockId};

fn timespec_to_ns(sec: i64, nsec: i64) -> Option<u64> {
    u64::try_from(sec)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(nsec).ok()?)
}

/// `CLOCK_MONOTONIC` in nanoseconds.
pub(super) fn monotonic_ns() -> Result<u64, String> {
    let ts = clock_gettime(ClockId::CLOCK_MONOTONIC)
        .map_err(|e| format!("clock_gettime(CLOCK_MONOTONIC) failed: {e}"))?;
    timespec_to_ns(ts.tv_sec(), ts.tv_nsec())
        .ok_or_else(|| "CLOCK_MONOTONIC value out of u64 nanosecond range".to_string())
}

/// True for exactly one caller per flag.
pub(super) fn claim(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::AcqRel)
}

/// The line and its newline in one `write_all` buffer, then a flush. Stderr is unbuffered, so
/// `writeln!` would issue two `write(2)` calls and a child process sharing the pipe (WebKit, GLib
/// warnings) could land between the text and its newline; one buffer is one `write(2)` for any
/// marker line (far below `PIPE_BUF`), which the kernel never interleaves.
pub(super) fn write_line_to(out: &mut impl Write, line: &str) -> std::io::Result<()> {
    let mut buf = String::with_capacity(line.len() + 1);
    buf.push_str(line);
    buf.push('\n');
    out.write_all(buf.as_bytes()).and_then(|()| out.flush())
}

/// Write one line to stderr under the stderr lock.
pub(super) fn write_line(line: &str) -> std::io::Result<()> {
    write_line_to(&mut std::io::stderr().lock(), line)
}

#[cfg(test)]
mod tests;
