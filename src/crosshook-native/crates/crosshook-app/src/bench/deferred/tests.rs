use super::*;
use crate::bench::plan::resolve_replay_plan;

fn plan() -> (tempfile::TempDir, ReplayPlan) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let log = root.join("launch.log");
    std::fs::write(&log, "one\n").unwrap();
    let plan = resolve_replay_plan(root.to_str(), log.to_str().unwrap(), Some("5000"), None)
        .expect("plan");
    (dir, plan)
}

#[test]
fn an_empty_slot_yields_nothing() {
    assert!(DeferredReplay::new().take().is_none());
}

#[test]
fn a_parked_plan_is_taken_exactly_once() {
    let (_dir, plan) = plan();
    let slot = DeferredReplay::new();
    slot.park(plan);
    let taken = slot.take().expect("parked plan");
    assert_eq!(taken.rate(), 5000);
    assert!(slot.take().is_none());
}

#[test]
fn the_public_take_is_inert_without_bench_mode() {
    // The unit-test process never sets CROSSHOOK_BENCH=1.
    assert!(!bench_enabled());
    assert!(take_deferred_replay().is_none());
}
