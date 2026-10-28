//! AC1–AC3: the catalog, not the caller, decides what gets installed.

use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpInstallErrorKind, ProtonUpInstallRequest,
};

use super::super::super::identity::IdentityError;
use super::super::super::messages::identity_message;
use super::super::helpers::{finished, ge_catalog, owner, request, Harness, Mode};
use crate::{AppError, AppErrorKind};

type Mutate = fn(&mut ProtonUpAvailableVersion);

fn case(name: &'static str, mutate: Mutate) -> (&'static str, Mutate) {
    (name, mutate)
}

fn catalog_entry() -> ProtonUpAvailableVersion {
    ge_catalog().versions.remove(0)
}

fn replace(url: &Option<String>, from: &str, to: &str) -> Option<String> {
    url.as_ref().map(|value| value.replace(from, to))
}

/// Supplied-entry variants that change the provider or version.
fn identity_cases() -> Vec<(&'static str, Mutate)> {
    vec![
        case("provider", |v| v.provider = "proton-em".to_string()),
        case("version", |v| v.version = "GE-Proton9-20".to_string()),
    ]
}

/// Supplied-entry variants that change download or checksum metadata.
fn metadata_cases() -> Vec<(&'static str, Mutate)> {
    vec![
        case("download host", |v| {
            v.download_url = replace(&v.download_url, "github.com", "evil.example");
        }),
        case("download path", |v| {
            v.download_url = replace(&v.download_url, "GE-Proton9-21.tar.gz", "other.tar.gz");
        }),
        case("download to none", |v| v.download_url = None),
        case("download query", |v| {
            v.download_url = v.download_url.as_ref().map(|url| format!("{url}?token=1"));
        }),
        case("download trailing slash", |v| {
            v.download_url = v.download_url.as_ref().map(|url| format!("{url}/"));
        }),
        case("checksum host", |v| {
            v.checksum_url = replace(&v.checksum_url, "github.com", "evil.example");
        }),
        case("checksum path", |v| {
            v.checksum_url = replace(&v.checksum_url, "GE-Proton9-21.sha512sum", "other.sum");
        }),
        case("checksum to none", |v| v.checksum_url = None),
        case("checksum query", |v| {
            v.checksum_url = v.checksum_url.as_ref().map(|url| format!("{url}?x=1"));
        }),
        case("checksum kind", |v| {
            v.checksum_kind = Some("sha256".to_string());
        }),
        case("checksum kind to none", |v| v.checksum_kind = None),
    ]
}

async fn start_error(
    harness: &Harness,
    request: ProtonUpInstallRequest,
    supplied: ProtonUpAvailableVersion,
) -> AppError {
    match harness
        .service
        .start_install(owner("main"), request, supplied)
        .await
    {
        Ok(_) => panic!("install request was accepted"),
        Err(error) => error,
    }
}

fn assert_validation(error: &AppError, expected: IdentityError, request: &ProtonUpInstallRequest) {
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "version" });
    assert_eq!(error.message(), identity_message(expected, request));
}

#[tokio::test]
async fn start_rejects_a_version_missing_from_the_catalog() {
    let harness = Harness::new(Mode::Succeed);
    let req = request("GE-Proton0-0");
    let mut supplied = catalog_entry();
    supplied.version = "GE-Proton0-0".to_string();
    let error = start_error(&harness, req.clone(), supplied).await;
    assert_validation(&error, IdentityError::Missing, &req);
    assert_eq!(
        error.message(),
        "requested version 'GE-Proton0-0' for provider 'ge-proton' was not found in the server catalog"
    );
    harness.assert_rejected();
}

#[tokio::test]
async fn start_rejects_a_request_for_another_provider() {
    let harness = Harness::new(Mode::Succeed);
    let mut req = request("GE-Proton9-21");
    req.provider = "proton-em".to_string();
    let error = start_error(&harness, req.clone(), catalog_entry()).await;
    assert_validation(&error, IdentityError::Missing, &req);
    harness.assert_rejected();
}

#[tokio::test]
async fn start_rejects_a_supplied_entry_with_another_identity() {
    for (name, mutate) in identity_cases() {
        let harness = Harness::new(Mode::Succeed);
        let req = request("GE-Proton9-21");
        let mut supplied = catalog_entry();
        mutate(&mut supplied);
        let error = start_error(&harness, req.clone(), supplied).await;
        assert_validation(&error, IdentityError::Mismatch, &req);
        assert_eq!(
            error.message(),
            "install request/version mismatch with server catalog identity",
            "{name}"
        );
        harness.assert_rejected();
    }
}

#[tokio::test]
async fn start_rejects_supplied_download_and_checksum_metadata_that_differs() {
    for (name, mutate) in metadata_cases() {
        let harness = Harness::new(Mode::Succeed);
        let req = request("GE-Proton9-21");
        let mut supplied = catalog_entry();
        mutate(&mut supplied);
        let error = start_error(&harness, req.clone(), supplied).await;
        assert_validation(&error, IdentityError::MetadataMismatch, &req);
        assert_eq!(
            error.message(),
            "install metadata did not match server-resolved catalog entry",
            "{name}"
        );
        harness.assert_rejected();
    }
}

#[tokio::test]
async fn start_installs_the_catalog_entry_not_the_caller_dto() {
    let harness = Harness::new(Mode::Succeed);
    let mut supplied = catalog_entry();
    // Fields the identity check deliberately ignores must never reach the installer.
    supplied.asset_size = Some(1);
    supplied.release_url = Some("https://evil.example/release".to_string());
    supplied.published_at = Some("1999-01-01T00:00:00Z".to_string());
    let started = harness
        .service
        .start_install(owner("main"), request("GE-Proton9-21"), supplied)
        .await;
    assert!(started.is_ok());
    finished(&harness.jobs).await;

    let seen = harness.installer.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let catalog = catalog_entry();
    assert_eq!(seen[0].asset_size, catalog.asset_size);
    assert_eq!(seen[0].release_url, catalog.release_url);
    assert_eq!(seen[0].published_at, catalog.published_at);
    assert_eq!(seen[0].download_url, catalog.download_url);
    assert_eq!(seen[0].checksum_url, catalog.checksum_url);
    assert_eq!(seen[0].checksum_kind, catalog.checksum_kind);
}

#[tokio::test]
async fn legacy_install_reports_a_missing_version_without_installing() {
    let harness = Harness::new(Mode::Succeed);
    let result = harness
        .service
        .install_version(request("GE-Proton0-0"))
        .await
        .unwrap();
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::DependencyMissing)
    );
    assert_eq!(
        result.error_message.as_deref(),
        Some("version GE-Proton0-0 not found in catalog")
    );
    harness.assert_rejected();
}

#[tokio::test]
async fn legacy_install_reports_another_provider_as_missing() {
    let harness = Harness::new(Mode::Succeed);
    let mut req = request("GE-Proton9-21");
    req.provider = "proton-em".to_string();
    let result = harness.service.install_version(req).await.unwrap();
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::DependencyMissing)
    );
    harness.assert_rejected();
}

#[tokio::test]
async fn legacy_install_installs_the_catalog_entry() {
    let harness = Harness::new(Mode::Succeed);
    let result = harness
        .service
        .install_version(request("GE-Proton9-21"))
        .await
        .unwrap();
    assert!(result.success);
    let seen = harness.installer.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].download_url, catalog_entry().download_url);
    assert_eq!(seen[0].asset_size, catalog_entry().asset_size);
}
