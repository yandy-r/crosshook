use super::*;

#[test]
fn decide_gates_on_exact_one() {
    assert_eq!(decide(None, None, None), BenchMode::Disabled);
    assert_eq!(
        decide(Some("0"), Some("1"), Some("/x")),
        BenchMode::Disabled
    );
    assert_eq!(decide(Some("true"), None, None), BenchMode::Disabled);
    assert_eq!(
        decide(Some("1"), None, None),
        BenchMode::Ready {
            exit_after_ready: false,
            replay: false
        }
    );
}

#[test]
fn decide_exit_flag_and_replay() {
    assert_eq!(
        decide(Some("1"), Some("1"), None),
        BenchMode::Ready {
            exit_after_ready: true,
            replay: false
        }
    );
    assert_eq!(
        decide(Some("1"), Some("0"), Some("/l")),
        BenchMode::Ready {
            exit_after_ready: false,
            replay: true
        }
    );
    // exit wins over replay
    assert_eq!(
        decide(Some("1"), Some("1"), Some("/l")),
        BenchMode::Ready {
            exit_after_ready: true,
            replay: false
        }
    );
}
