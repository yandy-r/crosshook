use super::*;
use crate::bench::plan::resolve_replay_plan;

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
