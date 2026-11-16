use super::*;

fn env(bench: &str) -> BenchEnv {
    BenchEnv {
        bench: Some(bench.to_string()),
        ..BenchEnv::default()
    }
}

#[test]
fn begin_disabled_does_not_claim() {
    let flag = AtomicBool::new(false);
    assert!(begin(&flag, &env("0")).unwrap().is_none());
    assert!(!flag.load(Ordering::SeqCst));
}

#[test]
fn begin_fires_once() {
    let flag = AtomicBool::new(false);
    let first = begin(&flag, &env("1")).unwrap().expect("first fires");
    assert!(first.ns > 0 && first.replay.is_none() && !first.exit_after_ready);
    assert!(begin(&flag, &env("1")).unwrap().is_none());
}

#[test]
fn begin_error_does_not_consume_one_shot() {
    let flag = AtomicBool::new(false);
    let mut bad = env("1");
    bad.log = Some("/nonexistent/launch.log".into()); // root missing
    assert!(begin(&flag, &bad).is_err());
    assert!(!flag.load(Ordering::SeqCst));
    assert!(begin(&flag, &env("1")).unwrap().is_some());
}

#[test]
fn exit_after_ready_is_carried_through() {
    let flag = AtomicBool::new(false);
    let mut exiting = env("1");
    exiting.exit_after_ready = Some("1".into());
    let started = begin(&flag, &exiting).unwrap().expect("fires");
    assert!(started.exit_after_ready && started.replay.is_none());
}

fn replay_env(scenario: Option<&str>) -> (tempfile::TempDir, BenchEnv) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let log = root.join("launch.log");
    std::fs::write(&log, "one\ntwo\n").unwrap();
    let mut replaying = env("1");
    replaying.root = root.to_str().map(str::to_string);
    replaying.log = log.to_str().map(str::to_string);
    replaying.scenario = scenario.map(str::to_string);
    (dir, replaying)
}

fn started(scenario: Option<&str>) -> (tempfile::TempDir, Started) {
    let (dir, replaying) = replay_env(scenario);
    let started = begin(&AtomicBool::new(false), &replaying)
        .unwrap()
        .expect("fires");
    (dir, started)
}

#[test]
fn only_replay_scroll_defers_the_replay() {
    for (scenario, defers) in [
        (None, false),
        (Some("search"), false),
        (Some("route_switch"), false),
        (Some("replay_scroll"), true),
        (Some("not_a_scenario"), false),
    ] {
        let (_dir, started) = started(scenario);
        assert!(started.replay.is_some(), "{scenario:?}");
        assert_eq!(started.defer_replay, defers, "{scenario:?}");
    }
}

#[test]
fn an_unknown_scenario_does_not_fail_ready() {
    let (_dir, replaying) = replay_env(Some("nope\nREADY 1"));
    assert!(begin(&AtomicBool::new(false), &replaying)
        .unwrap()
        .is_some());
}

#[test]
fn the_g4_path_hands_the_plan_to_the_shell_and_parks_nothing() {
    let (_dir, started) = started(None);
    let mut parked = false;
    let action = route_replay(started.replay, started.defer_replay, |_| parked = true);
    assert!(matches!(action, ReadyAction::Replay(_)), "{action:?}");
    assert!(!parked);
}

#[test]
fn a_deferring_scenario_parks_the_plan_and_returns_none() {
    let (_dir, started) = started(Some("replay_scroll"));
    let mut parked = None;
    let action = route_replay(started.replay, started.defer_replay, |plan| {
        parked = Some(plan);
    });
    assert!(matches!(action, ReadyAction::None), "{action:?}");
    assert_eq!(parked.expect("parked").rate(), 5000);
}

#[test]
fn nothing_is_parked_without_a_plan() {
    let mut parked = false;
    let action = route_replay(None, true, |_| parked = true);
    assert!(matches!(action, ReadyAction::None));
    assert!(!parked);
}
