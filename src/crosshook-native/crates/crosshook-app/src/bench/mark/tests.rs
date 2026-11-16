use super::*;

fn env(bench: Option<&str>, scenario: Option<&str>) -> BenchEnv {
    BenchEnv {
        bench: bench.map(str::to_string),
        scenario: scenario.map(str::to_string),
        ..BenchEnv::default()
    }
}

fn on(scenario: &str) -> BenchEnv {
    env(Some("1"), Some(scenario))
}

fn assert_one_line(line: &str) {
    assert!(!line.contains(['\n', '\r']), "{line:?}");
    assert!(!line.chars().any(char::is_control), "{line:?}");
    let mut out = Vec::new();
    write_line_to(&mut out, line).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.matches('\n').count(), 1);
    assert!(text.ends_with('\n'));
}

#[test]
fn disabled_gate_writes_nothing_and_skips_validation() {
    for bench in [None, Some("0"), Some("true"), Some("")] {
        let off = env(bench, Some("bad\nscenario"));
        // Garbage in every argument: the gate answers first.
        assert_eq!(
            check_mark(&off, "x\ny", u32::MAX, "nope", f64::NAN),
            Ok(None)
        );
        assert_eq!(check_clock(&off, u32::MAX, f64::NAN), Ok(None));
        let flag = AtomicBool::new(false);
        assert_eq!(plan_begin(&flag, &off), Ok(None));
        assert!(!flag.load(Ordering::SeqCst));
    }
}

#[test]
fn valid_mark_formats_from_enums_and_integers() {
    let mark = check_mark(&on("search"), "search", 7, "render", 1234.5678)
        .unwrap()
        .unwrap();
    let line = mark.line(42);
    assert_eq!(line, "BENCH_MARK 42 search 7 render 1234568");
    assert_one_line(&line);
}

#[test]
fn every_phase_and_scenario_formats() {
    for (phase, token) in [
        ("armed", "armed"),
        ("render", "render"),
        ("focus_lost", "focus_lost"),
    ] {
        let mark = check_mark(&on("route_switch"), "route_switch", 0, phase, 0.0)
            .unwrap()
            .unwrap();
        assert_eq!(
            mark.line(1),
            format!("BENCH_MARK 1 route_switch 0 {token} 0")
        );
    }
}

#[test]
fn scenario_must_match_the_active_one() {
    let long = "a".repeat(33);
    let bad = [
        "toggle_favorite",
        "",
        " ",
        "search ",
        "search\n",
        "search\r",
        "READY 1",
        "search\nREADY 1",
        long.as_str(),
        "séarch",
        "Search",
    ];
    for scenario in bad {
        let error = check_mark(&on("search"), scenario, 1, "render", 1.0).unwrap_err();
        assert_eq!(error, "scenario does not match the active bench scenario");
    }
}

#[test]
fn mark_without_an_active_scenario_is_rejected() {
    let error = check_mark(&env(Some("1"), None), "search", 1, "render", 1.0).unwrap_err();
    assert_eq!(error, "no active bench scenario");
    assert!(check_mark(&on("bogus"), "bogus", 1, "render", 1.0).is_err());
}

#[test]
fn unknown_phases_are_rejected() {
    for phase in [
        "",
        "Render",
        "render ",
        "render\n",
        "focus-lost",
        "READY 1",
        "ready",
    ] {
        let error = check_mark(&on("dpad"), "dpad", 1, phase, 1.0).unwrap_err();
        assert_eq!(error, "unknown bench phase");
    }
}

#[test]
fn phase_serde_is_closed_snake_case() {
    let phase: Phase = serde_json::from_str("\"focus_lost\"").unwrap();
    assert_eq!(phase, Phase::FocusLost);
    assert!(serde_json::from_str::<Phase>("\"focusLost\"").is_err());
    assert!(serde_json::from_str::<Phase>("\"other\"").is_err());
}

#[test]
fn seq_bounds() {
    assert!(check_mark(&on("dpad"), "dpad", MAX_SEQ, "armed", 1.0).is_ok());
    for seq in [MAX_SEQ + 1, u32::MAX] {
        assert!(check_mark(&on("dpad"), "dpad", seq, "armed", 1.0).is_err());
        assert!(check_clock(&on("dpad"), seq, 1.0).is_err());
    }
}

#[test]
fn t_perf_ms_must_be_finite_and_bounded() {
    for bad in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.001,
        -1.0,
        86_400_000.001,
        f64::MAX,
    ] {
        assert!(t_perf_us(bad).is_err(), "{bad}");
        assert!(check_mark(&on("dpad"), "dpad", 1, "armed", bad).is_err());
        assert!(check_clock(&on("dpad"), 1, bad).is_err());
    }
    assert_eq!(t_perf_us(0.0), Ok(0));
    assert_eq!(t_perf_us(-0.0), Ok(0));
    assert_eq!(t_perf_us(1.0004), Ok(1000));
    assert_eq!(t_perf_us(1.0005), Ok(1001));
    assert_eq!(t_perf_us(86_400_000.0), Ok(86_400_000_000));
}

#[test]
fn validation_order_is_scenario_phase_seq_time() {
    let e = on("search");
    assert_eq!(
        check_mark(&e, "dpad", u32::MAX, "bad", f64::NAN).unwrap_err(),
        "scenario does not match the active bench scenario"
    );
    assert_eq!(
        check_mark(&e, "search", u32::MAX, "bad", f64::NAN).unwrap_err(),
        "unknown bench phase"
    );
    assert!(check_mark(&e, "search", u32::MAX, "armed", f64::NAN)
        .unwrap_err()
        .starts_with("seq must"));
    assert!(check_mark(&e, "search", 1, "armed", f64::NAN)
        .unwrap_err()
        .starts_with("t_perf_ms must"));
}

#[test]
fn clock_line_format() {
    let (seq, t_us) = check_clock(&on("search"), 3, 2.5).unwrap().unwrap();
    let line = format_clock(99, seq, t_us);
    assert_eq!(line, "BENCH_CLOCK 99 3 2500");
    assert_one_line(&line);
    // The clock sample does not need a scenario.
    assert!(check_clock(&env(Some("1"), None), 0, 0.0)
        .unwrap()
        .is_some());
}

#[test]
fn begin_is_a_one_shot() {
    let flag = AtomicBool::new(false);
    let line = plan_begin(&flag, &on("toggle_optimization"))
        .unwrap()
        .expect("first call fires");
    assert!(line.starts_with("BENCH_BEGIN ") && line.ends_with(" toggle_optimization"));
    assert_one_line(&line);
    assert_eq!(plan_begin(&flag, &on("toggle_optimization")), Ok(None));
}

#[test]
fn begin_errors_leave_the_one_shot_unclaimed() {
    let flag = AtomicBool::new(false);
    assert!(plan_begin(&flag, &env(Some("1"), None)).is_err());
    assert!(plan_begin(&flag, &on("bogus")).is_err());
    assert!(!flag.load(Ordering::SeqCst));
    assert!(plan_begin(&flag, &on("search")).unwrap().is_some());
}

#[test]
fn cap_writes_up_to_cap_then_one_error_then_drops() {
    let budget = Budget::new();
    let mut out = Vec::new();
    for i in 0..3 {
        assert!(emit(&mut out, &budget, 3, &format!("BENCH_CLOCK {i} 0 0")).unwrap());
    }
    for _ in 0..5 {
        assert!(!emit(&mut out, &budget, 3, "BENCH_CLOCK 9 0 0").unwrap());
    }
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[3], "BENCH_ERROR: marker cap reached");
    assert!(!text.contains("BENCH_CLOCK 9"));
    assert_eq!(text.matches('\n').count(), 4);
}

#[test]
fn reported_clock_ns_is_the_stamp_only_for_a_written_line() {
    assert_eq!(reported_clock_ns(123, true), 123);
    assert_eq!(reported_clock_ns(123, false), 0);
}

#[test]
fn a_clock_sample_past_the_cap_reports_zero() {
    let budget = Budget::new();
    let mut out = Vec::new();
    let reported: Vec<u64> = (1..=4)
        .map(|ns| {
            let written = emit(&mut out, &budget, 2, &format!("BENCH_CLOCK {ns} 0 0")).unwrap();
            reported_clock_ns(ns, written)
        })
        .collect();
    // Two written, then the cap-reached line and a drop: neither reports a stamp.
    assert_eq!(reported, [1, 2, 0, 0]);
}

#[test]
fn budget_admit_sequence() {
    let budget = Budget::new();
    assert_eq!(budget.admit(1), Admit::Write);
    assert_eq!(budget.admit(1), Admit::CapReached);
    assert_eq!(budget.admit(1), Admit::Drop);
    assert_eq!(budget.admit(1), Admit::Drop);
}

#[test]
fn public_calls_are_inert_without_the_gate() {
    // The test process does not set CROSSHOOK_BENCH.
    assert_eq!(bench_begin(), Ok(()));
    assert_eq!(bench_mark("x\ny", u32::MAX, "bad", f64::NAN), Ok(()));
    assert_eq!(bench_clock(u32::MAX, f64::NAN), Ok(0));
    assert!(!BEGIN_FIRED.load(Ordering::SeqCst));
}
