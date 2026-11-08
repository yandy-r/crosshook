use super::*;

fn env(bench: &str) -> BenchEnv {
    BenchEnv {
        bench: Some(bench.to_string()),
        ..BenchEnv::default()
    }
}

#[test]
fn claim_is_one_shot() {
    let flag = AtomicBool::new(false);
    assert!(claim(&flag));
    assert!(!claim(&flag));
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

#[test]
fn timespec_conversion_is_checked() {
    assert_eq!(timespec_to_ns(2, 5), Some(2_000_000_005));
    assert_eq!(timespec_to_ns(-1, 0), None);
    assert_eq!(timespec_to_ns(0, -1), None);
    assert_eq!(timespec_to_ns(i64::MAX, 0), None);
}
