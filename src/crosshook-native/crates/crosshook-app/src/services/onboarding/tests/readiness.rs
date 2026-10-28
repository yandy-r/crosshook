use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::services::onboarding::LegacyNag;

#[tokio::test]
async fn generalized_readiness_persists_raw_snapshot_before_dismissals_and_sanitizing() {
    let tmp = tempfile::tempdir().unwrap();
    let settings = SettingsStore::with_base_path(tmp.path().to_path_buf());
    let metadata = MetadataStore::open_in_memory().unwrap();
    let probe = fake_probe();
    let service = OnboardingService::with_probe(
        settings,
        metadata.clone(),
        JobRegistry::default(),
        probe.clone(),
    );
    service
        .dismiss_legacy_nag(LegacyNag::UmuInstall)
        .await
        .unwrap();
    service
        .dismiss_legacy_nag(LegacyNag::SteamDeckCaveats)
        .await
        .unwrap();

    let result = service.check_generalized_readiness().await.unwrap();
    let row = metadata.get_host_readiness_snapshot().unwrap().unwrap();
    let raw: Vec<HostToolCheckResult> = serde_json::from_str(&row.tool_results_json).unwrap();
    assert_eq!(raw, probe.generalized.tool_checks);
    assert!(raw[0].install_guidance.is_some());
    assert_eq!(row.critical_failures, 1);
    assert_eq!(row.warnings, 1);
    assert!(result.tool_checks[0].install_guidance.is_none());
    assert!(result.umu_install_guidance.is_none());
    assert!(result.steam_deck_caveats.is_none());
    assert!(result.checks[0].remediation.is_empty());
    assert_eq!(result.checks[0].path, "~/.local/bin/umu-run");
    assert_eq!(probe.generalized_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn capabilities_uses_cached_snapshot_without_probing() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    let raw = readiness_result();
    metadata
        .upsert_host_readiness_snapshot(
            &raw.tool_checks,
            &raw.detected_distro_family,
            raw.all_passed,
            raw.critical_failures,
            raw.warnings,
        )
        .unwrap();
    let probe = fake_probe();
    let service = OnboardingService::with_probe(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        metadata,
        JobRegistry::default(),
        probe.clone(),
    );
    service.capabilities().await.unwrap();
    assert_eq!(probe.generalized_calls.load(Ordering::SeqCst), 0);
    assert_eq!(probe.system_calls.load(Ordering::SeqCst), 0);
    assert_eq!(probe.details_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn check_readiness_applies_steam_deck_caveats_dismissal_without_snapshot_write() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        metadata.clone(),
    );
    assert!(service
        .check_readiness()
        .await
        .unwrap()
        .steam_deck_caveats
        .is_some());
    service
        .dismiss_legacy_nag(LegacyNag::SteamDeckCaveats)
        .await
        .unwrap();
    assert!(service
        .check_readiness()
        .await
        .unwrap()
        .steam_deck_caveats
        .is_none());
    assert!(metadata.get_host_readiness_snapshot().unwrap().is_none());
}

#[tokio::test]
async fn generalized_readiness_fails_when_raw_snapshot_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    metadata
        .with_sqlite_conn("test dropping snapshots", |conn| {
            conn.execute_batch("DROP TABLE host_readiness_snapshots")?;
            Ok(())
        })
        .unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        metadata,
    );
    assert!(service.check_generalized_readiness().await.is_err());
}

#[test]
fn readiness_yields_while_host_probe_is_gated() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (probe, gate) = FakeHostProbe::gated(readiness_result(), readiness_result(), details());
    let gate = Arc::new(gate);
    let service = OnboardingService::with_probe(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        MetadataStore::disabled(),
        JobRegistry::default(),
        Arc::new(probe),
    );
    let watchdog_gate = gate.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            watchdog_gate.release();
        }
    });
    runtime.block_on(async {
        let task = tokio::spawn(async move { service.check_generalized_readiness().await });
        gate.entered().await;
        let yielded = !task.is_finished();
        gate.release();
        done_tx.send(()).unwrap();
        task.await.unwrap().unwrap();
        assert!(yielded, "readiness probe blocked the async caller");
    });
    watchdog.join().unwrap();
}
