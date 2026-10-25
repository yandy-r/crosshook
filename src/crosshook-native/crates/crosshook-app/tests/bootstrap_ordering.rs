//! Real subprocess instrumentation checks (each scenario runs in a re-executed child).
use crosshook_app::bootstrap::{
    pre_runtime, pre_runtime_misordered_with_host, BootstrapOptions, EnvPolicy, LoggingMode,
    PreRuntimeEnv, PreRuntimeOptions, SystemHost, UmuRefresh,
};
use crosshook_app::App;
use crosshook_core::platform::trace::{self, TraceEvent};
use std::process::Command;

struct Policy;
impl EnvPolicy for Policy {
    fn apply(&self, env: &mut PreRuntimeEnv<'_>) {
        env.set("CROSSHOOK_ORDERING_SET", "1");
        env.remove("CROSSHOOK_ORDERING_REMOVE");
    }
}
const TEST_NAME: &str = "bootstrap_ordering";

/// What the libtest-style command line asks this `harness = false` binary to do.
enum Invocation {
    Scenario(String),
    List,
    Skip,
    Run,
}

/// Interprets the subset of libtest arguments cargo, nextest and IDEs pass.
/// Only a bare run, or a filter that selects `bootstrap_ordering`, executes
/// the real-process scenarios; everything else must exit 0 without running.
fn parse_args(args: &[String]) -> Invocation {
    if args.get(1).map(String::as_str) == Some("--scenario") {
        return Invocation::Scenario(args.get(2).cloned().unwrap_or_default());
    }
    let (mut list, mut ignored, mut exact) = (false, false, false);
    let (mut filters, mut skips) = (Vec::new(), Vec::new());
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--list" => list = true,
            "--ignored" => ignored = true,
            "--exact" => exact = true,
            "--skip" => skips.extend(rest.next().cloned()),
            "--test-threads" | "--format" | "--color" | "--logfile" | "--report-time"
            | "--shuffle-seed" | "--ensure-time" | "-Z" => {
                rest.next();
            }
            flag if flag.starts_with('-') => {}
            filter => filters.push(filter.to_owned()),
        }
    }
    let selected = filters.is_empty()
        || filters.iter().any(|filter| {
            if exact {
                filter == TEST_NAME
            } else {
                TEST_NAME.contains(filter.as_str())
            }
        });
    let skipped = skips.iter().any(|skip| {
        if exact {
            skip == TEST_NAME
        } else {
            TEST_NAME.contains(skip.as_str())
        }
    });
    if list {
        return if ignored || !selected || skipped {
            Invocation::Skip
        } else {
            Invocation::List
        };
    }
    if ignored || !selected || skipped {
        Invocation::Skip
    } else {
        Invocation::Run
    }
}

/// `-Z <value>` is a libtest/rustc flag, not a test-name filter.
fn assert_z_flag_takes_a_value() {
    let args = |extra: &[&str]| {
        let mut args = vec!["bootstrap_ordering".to_owned()];
        args.extend(extra.iter().copied().map(str::to_owned));
        args
    };
    assert!(
        matches!(
            parse_args(&args(&["-Z", "unstable-options"])),
            Invocation::Run
        ),
        "-Z value must not be treated as a test filter"
    );
    assert!(matches!(
        parse_args(&args(&["--list", "-Z", "unstable-options"])),
        Invocation::List
    ));
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    match parse_args(&args) {
        Invocation::Scenario(scenario) => {
            child(&scenario).unwrap_or_else(|error| panic!("bootstrap ordering: {error}"));
        }
        Invocation::List => println!("{TEST_NAME}: test"),
        Invocation::Skip => {}
        Invocation::Run => {
            assert_z_flag_takes_a_value();
            let r4 = r4_rule(&synthetic_r4_violation());
            assert!(
                r4.as_ref().err().is_some_and(|e| e.starts_with("R4")),
                "synthetic R4 trace must be rejected by R4 alone, got {r4:?}"
            );
            let exe = std::env::current_exe().unwrap();
            for scenario in [
                "native",
                "flatpak-isolated",
                "flatpak-host-xdg",
                "misordered",
            ] {
                assert!(
                    Command::new(&exe)
                        .args(["--scenario", scenario])
                        .status()
                        .unwrap()
                        .success(),
                    "scenario {scenario} failed"
                );
            }
            println!("bootstrap ordering: all real-process scenarios passed");
        }
    }
}
fn child(scenario: &str) -> Result<(), String> {
    let root = tempfile::tempdir().map_err(|e| e.to_string())?;
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    // SAFETY: harness=false; no runtime or other thread exists in this child.
    unsafe {
        std::env::set_var("HOME", &home);
        for (key, rel) in [
            ("XDG_CONFIG_HOME", "sandbox/config"),
            ("XDG_DATA_HOME", "sandbox/data"),
            ("XDG_CACHE_HOME", "sandbox/cache"),
            ("XDG_STATE_HOME", "sandbox/state"),
        ] {
            std::env::set_var(key, home.join(rel));
        }
        for key in [
            "FLATPAK_ID",
            "CROSSHOOK_FLATPAK_HOST_XDG",
            "HOST_XDG_CONFIG_HOME",
            "HOST_XDG_DATA_HOME",
            "HOST_XDG_CACHE_HOME",
            "HOST_XDG_STATE_HOME",
        ] {
            std::env::remove_var(key);
        }
        if scenario != "native" {
            std::env::set_var("FLATPAK_ID", "dev.crosshook.CrossHook");
        }
        if scenario == "flatpak-host-xdg" {
            std::env::set_var("CROSSHOOK_FLATPAK_HOST_XDG", "1");
        }
    }
    if scenario == "flatpak-isolated" {
        let config = home.join(".config/crosshook");
        std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
        std::fs::write(
            config.join("settings.toml"),
            "onboarding_completed = true\n",
        )
        .map_err(|e| e.to_string())?;
    }
    trace::reset();
    let opts = PreRuntimeOptions::new().with_env_policy(Policy);
    let pre = if scenario == "misordered" {
        pre_runtime_misordered_with_host(opts, &mut SystemHost::new())
    } else {
        pre_runtime(opts)
    }
    .map_err(|e| e.to_string())?;
    if scenario == "flatpak-isolated" && !pre.migration().is_some_and(|m| m.imported_config) {
        return Err("isolated migration did not import settings".into());
    }
    if scenario == "flatpak-host-xdg"
        && std::env::var_os("XDG_CONFIG_HOME") != Some(home.join(".config").into_os_string())
    {
        return Err("XDG override missing".into());
    }
    let order = pre.steps().verify_pre_runtime_order();
    if scenario == "misordered" {
        if order.is_ok() {
            return Err("misordered step log accepted".into());
        }
    } else {
        order.map_err(|e| e.to_string())?;
    }
    let (app, _) = App::bootstrap(
        pre,
        BootstrapOptions::new()
            .with_logging(LoggingMode::Skip)
            .with_umu_refresh(UmuRefresh::Skip {
                reason: "ordering test",
            }),
    )
    .map_err(|e| e.to_string())?;
    let violations = check_trace(&trace::take());
    if scenario == "misordered" {
        // The step order is the primary defect here, so R2 must be among the
        // rules that reject the trace; other rules may fire as consequences.
        if !violations.iter().any(|v| v.starts_with("R2")) {
            return Err(format!(
                "misordered trace was not rejected by R2: {violations:?}"
            ));
        }
        println!("misordered rejected as required: {violations:?}");
    } else if !violations.is_empty() {
        return Err(violations.join("; "));
    }
    drop(app);
    Ok(())
}
fn mark_position(events: &[TraceEvent], name: &str) -> Result<usize, String> {
    events
        .iter()
        .position(|e| matches!(e, TraceEvent::Mark(m) if m == name))
        .ok_or_else(|| format!("missing mark {name}"))
}

/// Evaluates every rule independently and returns each violation, so the
/// rejecting rule of a bad trace is observable rather than whichever rule
/// happened to short-circuit first.
fn check_trace(events: &[TraceEvent]) -> Vec<String> {
    [
        r1_rule(events),
        r2_rule(events),
        r3_rule(events),
        r4_rule(events),
        r5_rule(events),
    ]
    .into_iter()
    .filter_map(Result::err)
    .collect()
}

/// R1: every environment mutation happens with exactly one thread.
fn r1_rule(events: &[TraceEvent]) -> Result<(), String> {
    for event in events {
        if let TraceEvent::EnvMutation { key, threads } = event {
            if *threads != Some(1) {
                return Err(format!("R1: mutation {key} with {threads:?} threads"));
            }
        }
    }
    Ok(())
}

/// R2: the Phase A step marks appear in the production order.
fn r2_rule(events: &[TraceEvent]) -> Result<(), String> {
    let mut previous = None;
    for name in [
        "gate:begin",
        "gate:end",
        "flatpak_migration:begin",
        "flatpak_migration:end",
        "legacy_app_id:begin",
        "legacy_app_id:end",
        "env_policy:begin",
        "env_policy:end",
        "pre:done",
    ] {
        let index = mark_position(events, name).map_err(|e| format!("R2: {e}"))?;
        if previous.is_some_and(|before| index <= before) {
            return Err(format!("R2: mark {name} out of order"));
        }
        previous = Some(index);
    }
    Ok(())
}

/// R3: no base-directory lookup precedes the end of the Flatpak gate.
fn r3_rule(events: &[TraceEvent]) -> Result<(), String> {
    let gate = mark_position(events, "gate:end").map_err(|e| format!("R3: {e}"))?;
    for (index, event) in events.iter().enumerate() {
        if matches!(event, TraceEvent::BaseDirsLookup { .. }) && index <= gate {
            return Err("R3: lookup before gate".into());
        }
    }
    Ok(())
}

/// R4: lookups made by the legacy app-id migration only happen after the
/// Flatpak migration finished. Independent of R2: it reads the marks itself.
fn r4_rule(events: &[TraceEvent]) -> Result<(), String> {
    let migration =
        mark_position(events, "flatpak_migration:end").map_err(|e| format!("R4: {e}"))?;
    let legacy_begin =
        mark_position(events, "legacy_app_id:begin").map_err(|e| format!("R4: {e}"))?;
    let legacy_end = mark_position(events, "legacy_app_id:end").map_err(|e| format!("R4: {e}"))?;
    for (index, event) in events.iter().enumerate() {
        if matches!(event, TraceEvent::BaseDirsLookup { .. })
            && index > legacy_begin
            && index < legacy_end
            && index < migration
        {
            return Err("R4: legacy lookup before Flatpak migration".into());
        }
    }
    Ok(())
}

/// R5: Phase B performed real lookups after `pre:done`.
fn r5_rule(events: &[TraceEvent]) -> Result<(), String> {
    let done = mark_position(events, "pre:done").map_err(|e| format!("R5: {e}"))?;
    let lookups = events
        .iter()
        .enumerate()
        .filter(|(index, e)| *index > done && matches!(e, TraceEvent::BaseDirsLookup { .. }))
        .count();
    if lookups == 0 {
        return Err("R5: no real Phase B lookups exercised".into());
    }
    Ok(())
}

/// Hand-built trace that breaks only R4: legacy migration looks up base
/// directories before the Flatpak migration ends, while gate order (R3) and
/// thread counts (R1) stay valid. R2 and R5 are not evaluated by the caller.
fn synthetic_r4_violation() -> Vec<TraceEvent> {
    let mark = |name: &str| TraceEvent::Mark(name.to_owned());
    vec![
        mark("gate:begin"),
        mark("gate:end"),
        mark("legacy_app_id:begin"),
        TraceEvent::BaseDirsLookup { threads: Some(1) },
        mark("legacy_app_id:end"),
        mark("flatpak_migration:begin"),
        mark("flatpak_migration:end"),
    ]
}
