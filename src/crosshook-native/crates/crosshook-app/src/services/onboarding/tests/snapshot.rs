use super::*;
use crate::services::onboarding::snapshot::snapshot_row_to_payload;
use crosshook_core::metadata::HostReadinessSnapshotRow;

#[tokio::test]
async fn cached_host_readiness_snapshot_returns_none_for_disabled_store() {
    let tmp = tempfile::tempdir().unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        MetadataStore::disabled(),
    );
    assert!(service
        .cached_host_readiness_snapshot()
        .await
        .unwrap()
        .is_none());
}

#[test]
fn snapshot_row_to_payload_decodes_and_sanitizes_resolved_paths() {
    let row = HostReadinessSnapshotRow {
        detected_distro_family: "Arch".into(),
        tool_results_json: serde_json::to_string(&vec![tool_check()]).unwrap(),
        all_passed: false,
        critical_failures: 1,
        warnings: 1,
        checked_at: "2026-04-16T10:00:00Z".into(),
    };
    let payload = snapshot_row_to_payload(row).unwrap();
    assert_eq!(
        payload.tool_checks[0].resolved_path.as_deref(),
        Some("~/.local/bin/umu-run")
    );
    assert_eq!(payload.critical_failures, 1);
    assert_eq!(payload.warnings, 1);
}

#[tokio::test]
async fn cached_snapshot_removes_dismissed_tool_guidance_without_mutating_raw_row() {
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
    metadata.dismiss_readiness_nag("umu_run", 90).unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        metadata.clone(),
    );
    let cached = service
        .cached_host_readiness_snapshot()
        .await
        .unwrap()
        .unwrap();
    assert!(cached.tool_checks[0].install_guidance.is_none());
    let row = metadata.get_host_readiness_snapshot().unwrap().unwrap();
    let stored: Vec<HostToolCheckResult> = serde_json::from_str(&row.tool_results_json).unwrap();
    assert!(stored[0].install_guidance.is_some());
}
