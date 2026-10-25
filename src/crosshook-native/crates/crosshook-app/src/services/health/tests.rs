use super::*;

#[tokio::test]
async fn batch_and_cached_snapshots_work_without_metadata() {
    let root = tempfile::tempdir().unwrap();
    let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
    let service = HealthService::new(profiles, MetadataStore::disabled());
    let summary = service.batch_summary().await.unwrap();
    assert_eq!(summary.total_count, 0);
    assert!(summary.profiles.is_empty());
    assert!(service.cached_health_snapshots().await.unwrap().is_empty());
    assert!(service
        .cached_offline_readiness_snapshots()
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn missing_profile_error_stays_sanitized() {
    let root = tempfile::tempdir().unwrap();
    let service = HealthService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
    );
    let error = service.profile_health("missing".into()).await.unwrap_err();
    assert!(!error.to_string().contains("/home/"));
}

#[test]
fn health_wire_shape_preserves_flattening_and_omission() {
    let report = EnrichedProfileHealthReport {
        core: crosshook_core::profile::health::ProfileHealthReport {
            name: "sample".into(),
            status: crosshook_core::profile::health::HealthStatus::Healthy,
            launch_method: "proton_run".into(),
            issues: vec![],
            checked_at: "now".into(),
        },
        metadata: None,
        offline_readiness: None,
    };
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["name"], "sample");
    assert_eq!(value["status"], "healthy");
    assert!(value["metadata"].is_null());
    assert!(value.get("core").is_none());
    assert!(value.get("offline_readiness").is_none());
}
