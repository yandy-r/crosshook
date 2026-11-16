use super::*;
use crate::bench::plan::resolve_replay_plan;
use std::cell::RefCell;

fn plan_for(text: &str) -> (tempfile::TempDir, ReplayPlan) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let log = root.join("launch.log");
    std::fs::write(&log, text).unwrap();
    let plan = resolve_replay_plan(root.to_str(), log.to_str().unwrap(), Some("5000"), None)
        .expect("plan");
    (dir, plan)
}

/// `BENCH_REPLAY <ns> <kind> <lines>` as `(kind, lines)`; panics on any other shape.
fn parse_marker(line: &str) -> (String, usize) {
    let fields: Vec<_> = line.split(' ').collect();
    assert_eq!(fields.len(), 4, "{line}");
    assert_eq!(fields[0], "BENCH_REPLAY");
    assert!(fields[1].parse::<u64>().unwrap() > 0, "{line}");
    (fields[2].to_string(), fields[3].parse().unwrap())
}

#[test]
fn due_lines_follow_the_rate_and_are_capped() {
    assert_eq!(due_lines(1000, 0, 10), 0);
    assert_eq!(due_lines(1000, 5, 10), 5);
    assert_eq!(due_lines(1000, 500, 10), 10);
    assert_eq!(due_lines(5000, 10, 100), 50);
}

#[test]
fn replay_lines_skip_blank_lines_and_cap_the_count() {
    assert_eq!(replay_lines("a\n\nb\n"), vec!["a", "b"]);
    let many = "x\n".repeat(MAX_REPLAY_LINES + 5);
    assert_eq!(replay_lines(&many).len(), MAX_REPLAY_LINES);
}

#[tokio::test]
async fn replay_emits_every_line_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let log = root.join("launch.log");
    std::fs::write(&log, "one\n\ntwo\nthree\n").unwrap();
    let plan = resolve_replay_plan(root.to_str(), log.to_str().unwrap(), Some("5000"), None)
        .expect("plan");
    let mut seen = Vec::new();
    replay_log(plan, |event| {
        assert_eq!(event.name, "launch-log");
        seen.push(event.value());
        true
    })
    .await;
    assert_eq!(
        seen,
        ["one", "two", "three"]
            .iter()
            .map(|line| serde_json::json!({ "line": line, "session_id": "bench" }))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn replay_stops_when_the_shell_cannot_deliver() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let log = root.join("launch.log");
    std::fs::write(&log, "one\ntwo\nthree\n").unwrap();
    let plan = resolve_replay_plan(root.to_str(), log.to_str().unwrap(), Some("5000"), None)
        .expect("plan");
    let mut calls = 0;
    replay_log(plan, |_| {
        calls += 1;
        false
    })
    .await;
    assert_eq!(calls, 1);
}

#[test]
fn marker_lines_are_integer_only_single_lines() {
    let line = format_replay_marker(123, "start", 50_000);
    assert_eq!(line, "BENCH_REPLAY 123 start 50000");
    assert_eq!(format_replay_marker(9, "done", 0), "BENCH_REPLAY 9 done 0");
}

#[test]
fn the_go_failure_line_names_the_code() {
    assert_eq!(
        go_failure_line("stdin closed before the GO line"),
        "BENCH_ERROR: GO_SIGNAL_FAILED: stdin closed before the GO line"
    );
}

#[tokio::test]
async fn a_deferred_replay_brackets_the_lines_with_start_and_done() {
    let (_dir, plan) = plan_for("one\n\ntwo\nthree\n");
    let trace = RefCell::new(Vec::new());
    run_replay(
        plan,
        |event| {
            trace
                .borrow_mut()
                .push(format!("emit {}", event.value()["line"]));
            true
        },
        true,
        |line| trace.borrow_mut().push(line.to_string()),
    )
    .await;
    let trace = trace.into_inner();
    assert_eq!(trace.len(), 5, "{trace:?}");
    assert_eq!(parse_marker(&trace[0]), ("start".to_string(), 3));
    assert_eq!(trace[1], "emit \"one\"");
    assert_eq!(trace[3], "emit \"three\"");
    assert_eq!(parse_marker(&trace[4]), ("done".to_string(), 3));
}

#[tokio::test]
async fn the_marker_times_are_ordered() {
    let (_dir, plan) = plan_for("a\nb\n");
    let mut seen = Vec::new();
    run_replay(plan, |_| true, true, |line| seen.push(line.to_string())).await;
    let ns: Vec<u64> = seen
        .iter()
        .map(|line| line.split(' ').nth(1).unwrap().parse().unwrap())
        .collect();
    assert_eq!(ns.len(), 2);
    assert!(ns[0] <= ns[1]);
}

#[tokio::test]
async fn an_empty_log_still_writes_start_and_done() {
    let (_dir, plan) = plan_for("\n\n");
    let mut seen = Vec::new();
    run_replay(plan, |_| true, true, |line| seen.push(line.to_string())).await;
    let kinds: Vec<_> = seen.iter().map(|l| parse_marker(l)).collect();
    assert_eq!(kinds, [("start".to_string(), 0), ("done".to_string(), 0)]);
}

#[tokio::test]
async fn the_g4_replay_writes_no_markers() {
    let (_dir, plan) = plan_for("one\ntwo\n");
    let mut markers = Vec::new();
    let mut emitted = 0;
    // `replay_log` writes to the real stderr, so exercise the same entry with a capturing sink.
    run_replay(
        plan,
        |_| {
            emitted += 1;
            true
        },
        false,
        |line| markers.push(line.to_string()),
    )
    .await;
    assert_eq!(emitted, 2);
    assert!(markers.is_empty(), "{markers:?}");
}

#[tokio::test]
async fn an_aborted_deferred_replay_writes_start_but_no_done() {
    let (_dir, plan) = plan_for("one\ntwo\nthree\n");
    let mut seen = Vec::new();
    run_replay(plan, |_| false, true, |line| seen.push(line.to_string())).await;
    assert_eq!(seen.len(), 1);
    assert_eq!(parse_marker(&seen[0]), ("start".to_string(), 3));
}

#[tokio::test]
async fn a_failed_go_signal_reports_once_and_replays_nothing() {
    let (_dir, plan) = plan_for("one\n");
    let mut seen = Vec::new();
    let mut emitted = 0;
    after_go(
        plan,
        Err("stdin closed before the GO line".to_string()),
        |_| {
            emitted += 1;
            true
        },
        |line| seen.push(line.to_string()),
    )
    .await;
    assert_eq!(emitted, 0);
    assert_eq!(
        seen,
        ["BENCH_ERROR: GO_SIGNAL_FAILED: stdin closed before the GO line"]
    );
}

#[tokio::test]
async fn a_received_go_signal_runs_the_bracketed_replay() {
    let (_dir, plan) = plan_for("one\ntwo\n");
    let mut seen = Vec::new();
    let mut emitted = 0;
    after_go(
        plan,
        Ok(()),
        |_| {
            emitted += 1;
            true
        },
        |line| seen.push(line.to_string()),
    )
    .await;
    assert_eq!(emitted, 2);
    let kinds: Vec<_> = seen.iter().map(|l| parse_marker(l)).collect();
    assert_eq!(kinds, [("start".to_string(), 2), ("done".to_string(), 2)]);
}
