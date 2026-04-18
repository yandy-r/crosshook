use super::super::{
    err, hex_encode, validate_archive_filename, validate_install_destination, validate_release_url,
    InstallError,
};
use crate::protonup::ProtonUpInstallErrorKind;

#[test]
fn rejects_empty_path() {
    let result = validate_install_destination("").unwrap_err();
    assert!(matches!(result, InstallError::InvalidPath(_)));
}

#[test]
fn cancelled_maps_to_cancelled_error_kind() {
    let result = InstallError::Cancelled.to_result();
    assert_eq!(result.error_kind, Some(ProtonUpInstallErrorKind::Cancelled));
    assert_eq!(result.error_message.as_deref(), Some("install cancelled"));
}

#[test]
fn rejects_path_with_parent_dir_component() {
    let result =
        validate_install_destination("/home/user/.steam/../../../etc/passwd/compatibilitytools.d")
            .unwrap_err();
    assert!(matches!(result, InstallError::InvalidPath(_)));
}

#[test]
fn rejects_path_without_compatibilitytools_d_segment() {
    let result = validate_install_destination("/home/user/.steam/root/steamapps").unwrap_err();
    assert!(matches!(result, InstallError::InvalidPath(_)));
}

#[test]
fn accepts_and_creates_valid_destination() {
    let temp = tempfile::tempdir().expect("temp dir");
    let dest = temp
        .path()
        .join("compatibilitytools.d")
        .to_string_lossy()
        .to_string();

    let result = validate_install_destination(&dest).unwrap();
    assert!(result.ends_with("compatibilitytools.d"));
    assert!(result.is_dir());
}

#[test]
fn validate_archive_filename_accepts_safe_name() {
    assert!(validate_archive_filename("GE-Proton9-21.tar.gz").is_ok());
}

#[test]
fn validate_archive_filename_rejects_unsafe_names() {
    for filename in [
        "",
        "../evil.tar.gz",
        "bad\\name.tar.gz",
        "bad/name.tar.gz",
        "bad\0name.tar.gz",
    ] {
        let result = validate_archive_filename(filename);
        assert!(
            matches!(result, Err(InstallError::InvalidPath(_))),
            "expected InvalidPath for {filename:?}, got {result:?}"
        );
    }
}

#[test]
fn rejects_symlink_redirect_escaping_compat_dir() {
    let temp = tempfile::tempdir().expect("temp dir");
    let real_target = temp.path().join("not_compat");
    std::fs::create_dir_all(&real_target).expect("real target dir");

    let symlink_path = temp.path().join("compatibilitytools.d");
    std::os::unix::fs::symlink(&real_target, &symlink_path).expect("create symlink");

    let dest = symlink_path.to_string_lossy().to_string();
    let result = validate_install_destination(&dest).unwrap_err();
    assert!(
        matches!(result, InstallError::InvalidPath(_)),
        "expected InvalidPath for symlink-redirected compat dir, got: {result:?}"
    );
}

#[test]
fn validate_release_url_accepts_known_github_hosts() {
    assert!(validate_release_url("https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton9-21/GE-Proton9-21.tar.gz").is_ok());
    assert!(validate_release_url(
        "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases"
    )
    .is_ok());
    assert!(validate_release_url("https://objects.githubusercontent.com/github-production-release-asset-2e65be/GE-Proton9-21.tar.gz").is_ok());
    assert!(validate_release_url(
        "https://github-releases.githubusercontent.com/GE-Proton9-21.tar.gz"
    )
    .is_ok());
}

#[test]
fn validate_release_url_rejects_http_scheme() {
    let result = validate_release_url("http://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton9-21/GE-Proton9-21.tar.gz");
    assert!(matches!(result, Err(InstallError::UntrustedUrl(_))));
}

#[test]
fn validate_release_url_rejects_untrusted_host() {
    let result = validate_release_url("https://evil.example.com/GE-Proton9-21.tar.gz");
    assert!(matches!(result, Err(InstallError::UntrustedUrl(_))));
}

#[test]
fn validate_release_url_rejects_malformed_url() {
    let result = validate_release_url("not a url at all");
    assert!(matches!(result, Err(InstallError::UntrustedUrl(_))));
}

#[test]
fn err_helper_sets_failure_fields() {
    let result = err("test message", ProtonUpInstallErrorKind::InvalidPath);
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::InvalidPath)
    );
    assert_eq!(result.error_message.as_deref(), Some("test message"));
    assert!(result.installed_path.is_none());
}

#[test]
fn hex_encode_produces_lowercase_hex() {
    let bytes = vec![0xde, 0xad, 0xbe, 0xef];
    assert_eq!(hex_encode(&bytes), "deadbeef");
}

#[test]
fn hex_encode_empty_bytes_gives_empty_string() {
    assert_eq!(hex_encode(&[]), "");
}

#[test]
fn hex_encode_single_byte() {
    assert_eq!(hex_encode(&[0xff]), "ff");
    assert_eq!(hex_encode(&[0x00]), "00");
}
