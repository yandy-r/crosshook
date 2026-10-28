use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use crosshook_core::onboarding::{HostToolDetails, ReadinessCheckResult};

use super::fake::FakeHostProbe;
use super::HostToolsService;
use crate::jobs::JobRegistry;

fn readiness() -> ReadinessCheckResult {
    ReadinessCheckResult {
        checks: Vec::new(),
        all_passed: true,
        critical_failures: 0,
        warnings: 0,
        umu_install_guidance: None,
        steam_deck_caveats: None,
        tool_checks: Vec::new(),
        detected_distro_family: String::new(),
    }
}

fn details(resolved_path: Option<String>) -> HostToolDetails {
    HostToolDetails {
        tool_id: "gamescope".to_string(),
        tool_version: Some("3.16.0".to_string()),
        resolved_path,
    }
}

#[tokio::test]
async fn tool_details_sanitizes_home_resolved_path() {
    let home = std::env::var("HOME").expect("test environment has HOME");
    let probe = Arc::new(FakeHostProbe::new(
        readiness(),
        readiness(),
        details(Some(format!("{home}/.local/bin/gamescope"))),
    ));
    let service = HostToolsService::with_probe(JobRegistry::default(), probe.clone());

    let result = service.tool_details("gamescope".to_string()).await.unwrap();

    assert_eq!(
        result.resolved_path.as_deref(),
        Some("~/.local/bin/gamescope")
    );
    assert_eq!(result.tool_id, "gamescope");
    assert_eq!(result.tool_version.as_deref(), Some("3.16.0"));
    assert_eq!(probe.details_calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.system_calls.load(Ordering::SeqCst), 0);
    assert_eq!(probe.generalized_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn tool_details_yields_while_probe_is_gated_and_reaps_job() {
    let (probe, gate) = FakeHostProbe::gated(readiness(), readiness(), details(None));
    let probe = Arc::new(probe);
    let gate = Arc::new(gate);
    let jobs = JobRegistry::default();
    let service = HostToolsService::with_probe(jobs.clone(), probe.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    let unblock = gate.clone();
    // Release on a synchronous regression without sleeps or runtime timers.
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            unblock.release();
        }
    });

    runtime.block_on(async {
        let task = tokio::spawn(async move { service.tool_details("gamescope".to_string()).await });
        gate.entered().await;
        let caller_can_progress = !task.is_finished();
        let running_counts = jobs.job_counts();
        let progress = tokio::spawn(async { "runtime made progress" })
            .await
            .unwrap();
        gate.release();
        done_tx.send(()).unwrap();
        let result = task.await.unwrap().unwrap();

        assert!(caller_can_progress, "probe blocked the caller thread");
        assert_eq!(progress, "runtime made progress");
        assert_eq!(running_counts, (1, 0));
        assert_eq!(jobs.job_counts(), (0, 0), "finished job was not reaped");
        assert_eq!(result, details(None));
        assert_eq!(probe.details_calls.load(Ordering::SeqCst), 1);
    });
    watchdog.join().unwrap();
}
