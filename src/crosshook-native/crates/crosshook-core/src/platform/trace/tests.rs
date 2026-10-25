//! Trace unit tests.
//!
//! Libtest runs other base-directory lookups in parallel, so the recording
//! tests filter their own marks out of the process-global log. They do not
//! require `Threads: 1`; the harness-false ordering test owns that check.
//!
//! Recording tests serialize on `TRACE_LOCK` because the log is
//! process-global: each test re-arms with `reset` and drains with `take`,
//! and concurrent tests would otherwise observe each other's arm windows.

use super::parse_thread_count;
#[cfg(feature = "bootstrap-trace")]
use super::TraceEvent;

#[test]
fn parse_thread_count_reads_the_threads_field() {
    let status = "Name:\tcrosshook\nThreads:\t7\nPid:\t1\n";
    assert_eq!(parse_thread_count(status), Some(7));
}

#[test]
fn parse_thread_count_trims_spaces() {
    assert_eq!(parse_thread_count("Threads:  12\n"), Some(12));
}

#[test]
fn parse_thread_count_rejects_missing_and_invalid_values() {
    assert_eq!(parse_thread_count("Name:\tfoo\n"), None);
    assert_eq!(parse_thread_count("Threads:\n"), None);
    assert_eq!(parse_thread_count("Threads: many\n"), None);
}

/// Serializes tests that arm and drain the process-global log.
#[cfg(feature = "bootstrap-trace")]
static TRACE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Max events accepted after one arm, from the parent module.
#[cfg(feature = "bootstrap-trace")]
const RECORD_LIMIT: usize = super::RECORD_LIMIT;

#[cfg(feature = "bootstrap-trace")]
#[test]
fn mark_env_and_lookup_are_filterable_from_the_global_log() {
    let _lock = TRACE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let begin = format!("trace-unit:begin:{}", std::process::id());
    let end = format!("trace-unit:end:{}", std::process::id());
    let key = format!("CROSSHOOK_TRACE_UNIT_{}", std::process::id());

    super::reset();
    super::mark(begin.as_str());
    super::env_mutation(key.as_str());
    super::base_dirs_lookup();
    super::mark(end.as_str());

    let events = super::take();
    let begin_at = mark_index(&events, &begin);
    let end_at = mark_index(&events, &end);
    assert!(begin_at < end_at, "begin mark must precede end mark");

    let env_at = events
        .iter()
        .position(|event| match event {
            TraceEvent::EnvMutation { key: recorded, .. } => recorded == &key,
            _ => false,
        })
        .unwrap_or_else(|| panic!("missing env mutation {key}"));
    assert!(begin_at < env_at && env_at < end_at);

    let TraceEvent::EnvMutation { threads, .. } = &events[env_at] else {
        panic!("env index {env_at} was not an env mutation");
    };
    assert_live_or_unknown(*threads);

    let lookup = events[env_at + 1..end_at]
        .iter()
        .find(|event| matches!(event, TraceEvent::BaseDirsLookup { .. }));
    let Some(TraceEvent::BaseDirsLookup { threads }) = lookup else {
        panic!("base-dir lookup must sit between the env mutation and the end mark");
    };
    assert_live_or_unknown(*threads);

    // `take` disarms: events recorded afterwards must not enter the log
    // until the next `reset` re-arms it.
    let leak = format!("trace-unit:leak:{}", std::process::id());
    super::mark(leak.as_str());
    let drained = super::take();
    assert!(
        !drained
            .iter()
            .any(|event| matches!(event, TraceEvent::Mark(recorded) if recorded == &leak)),
        "events recorded after take must not enter the log"
    );
    assert!(drained.is_empty(), "disarmed take must return nothing");
}

/// One arm keeps at most `RECORD_LIMIT` events, dropping the newest ones, so
/// a process that stays armed cannot grow the log without bound.
///
/// Other tests may call `base_dirs()` while this arm window is open and
/// interleave their own lookups. The assertions therefore only rely on
/// properties that hold for any number of foreign events: the log is filled
/// to the cap exactly, this test's own marks survive as a contiguous prefix
/// in push order, and the pushed tail is dropped.
#[cfg(feature = "bootstrap-trace")]
#[test]
fn recording_stops_at_the_record_limit_and_keeps_early_events() {
    let _lock = TRACE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let marker = format!("trace-unit:cap-marker:{}", std::process::id());
    // Pushed as this test's event #(RECORD_LIMIT + 2): at that point more
    // than `RECORD_LIMIT` events were already recorded, so it can never be
    // kept, no matter how many foreign events interleave.
    let tail = format!("trace-unit:cap-{RECORD_LIMIT}");

    super::reset();
    super::mark(marker.as_str());
    for index in 0..=(RECORD_LIMIT + 8) {
        super::mark(format!("trace-unit:cap-{index}").as_str());
    }

    let events = super::take();
    assert_eq!(events.len(), RECORD_LIMIT, "recording must stop at the cap");

    let kept: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            TraceEvent::Mark(recorded) if recorded.starts_with("trace-unit:cap-") => {
                Some(recorded.clone())
            }
            _ => None,
        })
        .collect();
    let pushed: Vec<String> = std::iter::once(marker)
        .chain((0..RECORD_LIMIT).map(|index| format!("trace-unit:cap-{index}")))
        .collect();
    assert!(
        pushed.starts_with(kept.as_slice()),
        "kept own marks must be the oldest, in push order"
    );
    assert!(
        !events.contains(&TraceEvent::Mark(tail)),
        "events past the record limit must be dropped"
    );
}

#[cfg(feature = "bootstrap-trace")]
fn mark_index(events: &[TraceEvent], name: &str) -> usize {
    events
        .iter()
        .position(|event| matches!(event, TraceEvent::Mark(recorded) if recorded == name))
        .unwrap_or_else(|| panic!("missing mark {name}"))
}

/// `/proc` may be unreadable (`None`). A live count is only bounded below by 1.
#[cfg(feature = "bootstrap-trace")]
fn assert_live_or_unknown(threads: Option<u32>) {
    if let Some(count) = threads {
        assert!(count >= 1, "recorded thread count {count} is below 1");
    }
}
