use super::*;
use std::sync::atomic::Ordering;

fn env(bench: Option<&str>, scenario: Option<&str>) -> BenchEnv {
    BenchEnv {
        bench: bench.map(str::to_string),
        scenario: scenario.map(str::to_string),
        ..BenchEnv::default()
    }
}

#[test]
fn every_allowlisted_name_round_trips() {
    let names: Vec<_> = ALL.into_iter().map(Scenario::name).collect();
    assert_eq!(
        names,
        [
            "search",
            "toggle_favorite",
            "toggle_optimization",
            "dpad",
            "route_switch",
            "replay_scroll"
        ]
    );
    for scenario in ALL {
        assert_eq!(Scenario::parse(scenario.name()), Some(scenario));
        assert!(scenario.name().len() <= 32);
    }
}

#[test]
fn disabled_mode_never_parses_or_errors() {
    for bench in [None, Some("0"), Some("true"), Some("")] {
        assert_eq!(parse_scenario(&env(bench, Some("search"))), Ok(None));
        assert_eq!(parse_scenario(&env(bench, Some("bad\nvalue"))), Ok(None));
    }
}

#[test]
fn enabled_without_scenario_is_none() {
    assert_eq!(parse_scenario(&env(Some("1"), None)), Ok(None));
}

#[test]
fn enabled_valid_scenario_is_returned() {
    assert_eq!(
        parse_scenario(&env(Some("1"), Some("dpad"))),
        Ok(Some(Scenario::Dpad))
    );
}

#[test]
fn invalid_scenarios_are_rejected_and_escaped() {
    let long = "a".repeat(33);
    let cases = [
        "",
        " ",
        "search ",
        "Search",
        "search\n",
        "search\r",
        "READY 1",
        "search\nREADY 1",
        long.as_str(),
        "séarch",
        "дпад",
        "\u{0}",
    ];
    for raw in cases {
        let error = parse_scenario(&env(Some("1"), Some(raw))).unwrap_err();
        assert!(error.starts_with("CROSSHOOK_BENCH_SCENARIO is not a known scenario"));
        assert!(
            !error.chars().any(char::is_control),
            "unescaped control byte in {error:?}"
        );
        assert!(error.contains(&format!("{raw:?}")));
    }
}

#[test]
fn error_is_reported_once_and_yields_none() {
    let flag = AtomicBool::new(false);
    let bad = parse_scenario(&env(Some("1"), Some("nope")));
    assert_eq!(report_once(&flag, bad.clone()), None);
    assert!(flag.load(Ordering::SeqCst));
    // A second failure does not re-claim; the result is still None.
    assert_eq!(report_once(&flag, bad), None);
    assert_eq!(
        report_once(&AtomicBool::new(false), Ok(Some(Scenario::Search))),
        Some(Scenario::Search)
    );
}

#[test]
fn public_scenario_serializes_its_name() {
    let json = serde_json::to_string(&Scenario::RouteSwitch.public()).unwrap();
    assert_eq!(json, r#"{"name":"route_switch"}"#);
}

#[test]
fn only_replay_scroll_defers_the_replay() {
    for scenario in ALL {
        assert_eq!(
            scenario.defers_replay(),
            scenario == Scenario::ReplayScroll,
            "{}",
            scenario.name()
        );
    }
}

#[test]
fn replay_scroll_is_selected_through_the_env() {
    assert_eq!(
        parse_scenario(&env(Some("1"), Some("replay_scroll"))),
        Ok(Some(Scenario::ReplayScroll))
    );
}
