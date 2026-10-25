//! Process-global bootstrap ordering trace.
//!
//! [`mark`], [`env_mutation`], and [`base_dirs_lookup`] append to one ordered
//! log **only while the trace is armed**: [`reset`] clears and arms the log,
//! and [`take`] drains and disarms it. The arm flag is an atomic, so the
//! disarmed path — the steady state of any long-lived process, including a
//! `--workspace` test binary where feature unification turns this crate's
//! `bootstrap-trace` feature on — returns before it touches the log mutex or
//! reads `/proc`. A producer that passed that fast check rechecks the flag
//! while holding the log mutex, so it cannot append after [`take`] has drained.
//! Only [`thread_count`] always reads `/proc`, because startup needs the live
//! `Threads:` count before any runtime exists.
//!
//! Recording is additionally capped: one arm accepts at most
//! `RECORD_LIMIT` events, so a process that stays armed cannot grow the
//! log without bound. The `bootstrap_ordering` harness arms with `reset`,
//! runs Phase A and Phase B, then drains with `take` — that sequence is the
//! only producer of observable events.
//!
//! The log is process-global. Libtest runs this crate's tests in parallel with
//! other base-directory lookups, so unit tests re-arm explicitly and filter
//! their own marks instead of requiring the whole log. The ordering
//! integration test runs in its own process and can assert on every event.

#[cfg(feature = "bootstrap-trace")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, MutexGuard,
};

/// One recorded bootstrap event, in process order.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEvent {
    /// A named step boundary (`"gate:begin"`, `"pre:done"`, …).
    Mark(String),
    /// An environment write or removal.
    EnvMutation {
        /// Environment variable name.
        key: String,
        /// [`thread_count`] at the mutation, or `None` if `/proc` was unreadable.
        threads: Option<u32>,
    },
    /// A [`crate::platform::base_dirs`] lookup.
    BaseDirsLookup {
        /// [`thread_count`] at the lookup, or `None` if `/proc` was unreadable.
        threads: Option<u32>,
    },
}

/// Maximum events kept after one [`reset`].
///
/// A full Phase A + Phase B bootstrap run records a few dozen events (marks
/// for every step boundary, one event per environment mutation, one per
/// base-directory lookup, including the lookups made by store initialization
/// and scheduled startup jobs). 256 covers that with a wide margin while
/// keeping a permanently armed process from appending forever.
#[cfg(feature = "bootstrap-trace")]
const RECORD_LIMIT: usize = 256;

/// Read `Threads:` from `/proc/self/status`.
///
/// Returns `None` when `/proc` cannot be read or the field is missing. The
/// value is whatever the kernel reports, including counts above one when a
/// test harness has already spawned workers.
#[doc(hidden)]
pub fn thread_count() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_thread_count(&status)
}

/// Parse the `Threads:` field out of a `/proc/self/status` body.
fn parse_thread_count(status: &str) -> Option<u32> {
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Threads:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

/// Append a named mark. No-op unless the trace is armed.
#[doc(hidden)]
#[inline]
pub fn mark(name: &str) {
    #[cfg(feature = "bootstrap-trace")]
    {
        if !armed() {
            return;
        }
        push(TraceEvent::Mark(name.to_owned()));
    }
    #[cfg(not(feature = "bootstrap-trace"))]
    {
        let _ = name;
    }
}

/// Record an environment write or removal, including the live thread count.
///
/// No-op (and no `/proc` read) unless the trace is armed.
#[doc(hidden)]
#[inline]
pub fn env_mutation(key: &str) {
    #[cfg(feature = "bootstrap-trace")]
    {
        if !armed() {
            return;
        }
        push(TraceEvent::EnvMutation {
            key: key.to_owned(),
            threads: thread_count(),
        });
    }
    #[cfg(not(feature = "bootstrap-trace"))]
    {
        let _ = key;
    }
}

/// Record a base-directory lookup, including the live thread count.
///
/// No-op (and no `/proc` read) unless the trace is armed.
#[doc(hidden)]
#[inline]
pub fn base_dirs_lookup() {
    #[cfg(feature = "bootstrap-trace")]
    {
        if !armed() {
            return;
        }
        push(TraceEvent::BaseDirsLookup {
            threads: thread_count(),
        });
    }
}

/// Whether the trace is currently armed (recording).
///
/// Loads the atomic flag only. Callers on the disarmed path return before
/// they take the log mutex.
#[cfg(feature = "bootstrap-trace")]
#[inline]
fn armed() -> bool {
    armed_flag().load(Ordering::Acquire)
}

/// Drain the process-global log and disarm the trace.
///
/// Returns an empty vector when `bootstrap-trace` is disabled or no arm is
/// outstanding.
#[doc(hidden)]
#[inline]
#[must_use]
pub fn take() -> Vec<TraceEvent> {
    #[cfg(feature = "bootstrap-trace")]
    {
        let mut guard = lock();
        // Disarm under the mutex, before the drain is published. A producer
        // that already passed the fast gate rechecks this flag while holding
        // the same mutex and drops the event instead of appending after take.
        armed_flag().store(false, Ordering::Release);
        std::mem::take(&mut guard.events)
    }
    #[cfg(not(feature = "bootstrap-trace"))]
    Vec::new()
}

/// Clear the process-global log and arm the trace.
///
/// No-op unless `bootstrap-trace` is enabled. This is the only way to start
/// recording; the `bootstrap_ordering` harness calls it before Phase A.
#[doc(hidden)]
#[inline]
pub fn reset() {
    #[cfg(feature = "bootstrap-trace")]
    {
        let mut guard = lock();
        guard.events.clear();
        // Arm only after the clear, still holding the mutex, so a producer
        // cannot append into the window `clear` is about to drop.
        armed_flag().store(true, Ordering::Release);
    }
}

#[cfg(feature = "bootstrap-trace")]
struct TraceLog {
    events: Vec<TraceEvent>,
}

#[cfg(feature = "bootstrap-trace")]
impl TraceLog {
    const fn new() -> Self {
        Self { events: Vec::new() }
    }
}

#[cfg(feature = "bootstrap-trace")]
fn armed_flag() -> &'static AtomicBool {
    static ARMED: AtomicBool = AtomicBool::new(false);
    &ARMED
}

#[cfg(feature = "bootstrap-trace")]
fn trace_log() -> &'static Mutex<TraceLog> {
    static LOG: Mutex<TraceLog> = Mutex::new(TraceLog::new());
    &LOG
}

#[cfg(feature = "bootstrap-trace")]
fn lock() -> MutexGuard<'static, TraceLog> {
    trace_log()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Append `event` while armed, dropping it when disarmed or over the cap.
///
/// The arm flag is rechecked under the log mutex so a producer that passed
/// the lock-free gate cannot append after [`take`] disarms and drains.
#[cfg(feature = "bootstrap-trace")]
fn push(event: TraceEvent) {
    let mut guard = lock();
    if !armed_flag().load(Ordering::Acquire) || guard.events.len() >= RECORD_LIMIT {
        return;
    }
    guard.events.push(event);
}

#[cfg(test)]
mod tests;
