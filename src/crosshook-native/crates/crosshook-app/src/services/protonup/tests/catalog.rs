use std::sync::atomic::Ordering;

use super::helpers::{ge_catalog, Harness, Mode};

fn last_args(harness: &Harness) -> Option<(String, bool, bool)> {
    harness.catalog.last_args.lock().unwrap().clone()
}

#[tokio::test]
async fn omitted_provider_defaults_to_ge_proton() {
    let harness = Harness::new(Mode::Succeed);
    let response = harness
        .service
        .list_available_versions(None, false)
        .await
        .unwrap();
    assert_eq!(response.versions.len(), 1);
    assert_eq!(
        last_args(&harness),
        Some(("ge-proton".to_string(), false, false))
    );
}

#[tokio::test]
async fn explicit_provider_and_force_refresh_are_passed_through() {
    let harness = Harness::new(Mode::Succeed);
    harness
        .service
        .list_available_versions(Some("proton-em".to_string()), true)
        .await
        .unwrap();
    assert_eq!(
        last_args(&harness),
        Some(("proton-em".to_string(), true, false))
    );
}

#[tokio::test]
async fn offline_mode_never_forces_a_refresh() {
    let harness = Harness::new(Mode::Succeed).with_offline();
    harness
        .service
        .list_available_versions(None, true)
        .await
        .unwrap();
    assert_eq!(
        last_args(&harness),
        Some(("ge-proton".to_string(), false, false))
    );
}

#[tokio::test]
async fn prerelease_flag_is_read_from_settings() {
    let harness = Harness::new(Mode::Succeed);
    let mut settings = harness.settings.load().unwrap();
    settings.protonup_include_prereleases = true;
    harness.settings.save(&settings).unwrap();
    harness
        .service
        .list_available_versions(None, false)
        .await
        .unwrap();
    assert_eq!(
        last_args(&harness),
        Some(("ge-proton".to_string(), false, true))
    );
}

#[tokio::test]
async fn stale_and_offline_markers_pass_through() {
    let harness = Harness::new(Mode::Succeed);
    let mut response = ge_catalog();
    response.cache.stale = true;
    response.cache.offline = true;
    response.cache.fetched_at = Some("2024-11-01T00:00:00Z".to_string());
    harness.catalog.set_response(response);

    let served = harness
        .service
        .list_available_versions(None, false)
        .await
        .unwrap();
    assert!(served.cache.stale);
    assert!(served.cache.offline);
    assert_eq!(
        served.cache.fetched_at.as_deref(),
        Some("2024-11-01T00:00:00Z")
    );
    assert_eq!(harness.catalog.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn providers_are_described_from_the_registry() {
    let harness = Harness::new(Mode::Succeed);
    let providers = harness.service.list_providers();
    assert!(providers.iter().any(|p| p.id == "ge-proton"));
}

#[tokio::test]
async fn unreadable_settings_never_force_a_refresh() {
    let harness = Harness::new(Mode::Succeed);
    let path = harness.settings.settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"offline_mode = [not valid toml").unwrap();

    harness
        .service
        .list_available_versions(None, true)
        .await
        .unwrap();

    assert_eq!(
        last_args(&harness),
        Some(("ge-proton".to_string(), false, false))
    );
}
