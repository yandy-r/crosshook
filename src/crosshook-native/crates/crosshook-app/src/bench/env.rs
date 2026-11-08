//! The `CROSSHOOK_BENCH*` environment contract and its pure decision.

pub(super) const ENV_BENCH: &str = "CROSSHOOK_BENCH";
pub(super) const ENV_EXIT: &str = "CROSSHOOK_BENCH_EXIT_AFTER_READY";
pub(super) const ENV_ROOT: &str = "CROSSHOOK_BENCH_ROOT";
pub(super) const ENV_LOG: &str = "CROSSHOOK_BENCH_LOG";
pub(super) const ENV_RATE: &str = "CROSSHOOK_BENCH_LOG_RATE";

/// True when benchmark mode is on (`CROSSHOOK_BENCH=1`).
pub fn bench_enabled() -> bool {
    std::env::var(ENV_BENCH).is_ok_and(|v| v == "1")
}

#[derive(Debug, Default)]
pub(super) struct BenchEnv {
    pub(super) bench: Option<String>,
    pub(super) exit_after_ready: Option<String>,
    pub(super) root: Option<String>,
    pub(super) log: Option<String>,
    pub(super) rate: Option<String>,
}

impl BenchEnv {
    pub(super) fn from_process() -> Self {
        let get = |k: &str| std::env::var(k).ok();
        Self {
            bench: get(ENV_BENCH),
            exit_after_ready: get(ENV_EXIT),
            root: get(ENV_ROOT),
            log: get(ENV_LOG),
            rate: get(ENV_RATE),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum BenchMode {
    Disabled,
    Ready {
        exit_after_ready: bool,
        replay: bool,
    },
}

/// Pure env decision. Replay only when a log is configured and we are not exiting.
pub(super) fn decide(
    bench: Option<&str>,
    exit_after_ready: Option<&str>,
    log: Option<&str>,
) -> BenchMode {
    if bench != Some("1") {
        return BenchMode::Disabled;
    }
    let exit_after_ready = exit_after_ready == Some("1");
    BenchMode::Ready {
        exit_after_ready,
        replay: log.is_some() && !exit_after_ready,
    }
}

#[cfg(test)]
mod tests;
