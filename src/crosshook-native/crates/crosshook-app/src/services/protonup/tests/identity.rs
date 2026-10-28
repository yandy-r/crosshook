use crosshook_core::protonup::{ProtonUpAvailableVersion, ProtonUpInstallRequest};

use super::super::identity::{verify_install_identity, IdentityError};
use super::helpers::{ge_catalog, request};

fn catalog() -> Vec<ProtonUpAvailableVersion> {
    ge_catalog().versions
}

fn supplied() -> ProtonUpAvailableVersion {
    catalog().remove(0)
}

fn verify(
    request: &ProtonUpInstallRequest,
    supplied: Option<&ProtonUpAvailableVersion>,
) -> Result<(), IdentityError> {
    let catalog = catalog();
    verify_install_identity(request, supplied, &catalog).map(|_| ())
}

#[test]
fn returns_the_catalog_entry_for_the_requested_identity() {
    let catalog = catalog();
    let entry = verify_install_identity(&request("GE-Proton9-21"), None, &catalog).unwrap();
    assert!(std::ptr::eq(entry, &catalog[0]));
}

#[test]
fn supplied_matching_entry_returns_the_catalog_entry_not_the_supplied_one() {
    let catalog = catalog();
    let supplied = supplied();
    let entry =
        verify_install_identity(&request("GE-Proton9-21"), Some(&supplied), &catalog).unwrap();
    assert!(std::ptr::eq(entry, &catalog[0]));
    assert!(!std::ptr::eq(entry, &supplied));
}

#[test]
fn missing_version_is_missing() {
    assert_eq!(
        verify(&request("GE-Proton0-0"), None),
        Err(IdentityError::Missing)
    );
    assert_eq!(
        verify(&request("GE-Proton0-0"), Some(&supplied())),
        Err(IdentityError::Missing)
    );
}

#[test]
fn request_provider_mismatch_is_missing() {
    let mut req = request("GE-Proton9-21");
    req.provider = "proton-em".to_string();
    assert_eq!(verify(&req, None), Err(IdentityError::Missing));
}

#[test]
fn empty_catalog_is_missing() {
    assert_eq!(
        verify_install_identity(&request("GE-Proton9-21"), None, &[]).map(|_| ()),
        Err(IdentityError::Missing)
    );
}

#[test]
fn supplied_provider_mismatch_is_mismatch() {
    let mut supplied = supplied();
    supplied.provider = "proton-em".to_string();
    assert_eq!(
        verify(&request("GE-Proton9-21"), Some(&supplied)),
        Err(IdentityError::Mismatch)
    );
}

#[test]
fn supplied_version_mismatch_is_mismatch() {
    let mut supplied = supplied();
    supplied.version = "GE-Proton9-20".to_string();
    assert_eq!(
        verify(&request("GE-Proton9-21"), Some(&supplied)),
        Err(IdentityError::Mismatch)
    );
}

#[test]
fn each_metadata_field_mutated_alone_is_metadata_mismatch() {
    let mutations: [(&str, fn(&mut ProtonUpAvailableVersion)); 9] = [
        ("download_url host", |v| {
            v.download_url = Some("https://evil.example/GE-Proton9-21.tar.gz".to_string());
        }),
        ("download_url path", |v| {
            v.download_url = v
                .download_url
                .take()
                .map(|u| u.replace(".tar.gz", ".tar.xz"));
        }),
        ("download_url query", |v| {
            v.download_url = v.download_url.take().map(|u| format!("{u}?x=1"));
        }),
        ("download_url trailing slash", |v| {
            v.download_url = v.download_url.take().map(|u| format!("{u}/"));
        }),
        ("download_url none", |v| v.download_url = None),
        ("checksum_url host", |v| {
            v.checksum_url = Some("https://evil.example/GE-Proton9-21.sha512sum".to_string());
        }),
        ("checksum_url none", |v| v.checksum_url = None),
        ("checksum_kind value", |v| {
            v.checksum_kind = Some("sha256".to_string());
        }),
        ("checksum_kind none", |v| v.checksum_kind = None),
    ];
    for (name, mutate) in mutations {
        let mut supplied = supplied();
        mutate(&mut supplied);
        assert_eq!(
            verify(&request("GE-Proton9-21"), Some(&supplied)),
            Err(IdentityError::MetadataMismatch),
            "{name}"
        );
    }
}

#[test]
fn none_supplied_metadata_against_some_catalog_metadata_is_not_checked() {
    // Legacy entry point: no supplied DTO means no metadata comparison.
    assert!(verify(&request("GE-Proton9-21"), None).is_ok());
}

#[test]
fn release_url_asset_size_and_published_at_are_not_compared() {
    let mut supplied = supplied();
    supplied.release_url = Some("https://evil.example/release".to_string());
    supplied.asset_size = Some(1);
    supplied.published_at = None;
    assert!(verify(&request("GE-Proton9-21"), Some(&supplied)).is_ok());
}

#[test]
fn first_matching_catalog_entry_wins() {
    let mut catalog = catalog();
    let mut second = catalog[0].clone();
    second.asset_size = Some(999);
    catalog.push(second);
    let entry = verify_install_identity(&request("GE-Proton9-21"), None, &catalog).unwrap();
    assert!(std::ptr::eq(entry, &catalog[0]));
}
