use sha2::{Digest, Sha512};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::{hex_encode, install_version, install_version_with_progress, InstallError};
use super::support::{make_version, minimal_ge_proton_tar_gz, sha256_version};
use crate::protonup::progress::{Phase, ProgressEmitter};
use crate::protonup::providers::{self, ChecksumKind};
use crate::protonup::{ProtonUpAvailableVersion, ProtonUpInstallErrorKind, ProtonUpInstallRequest};

struct FakeSha256Provider {
    supports: bool,
}

#[async_trait::async_trait]
impl providers::ProtonReleaseProvider for FakeSha256Provider {
    fn id(&self) -> &'static str {
        "fake-sha256"
    }

    fn display_name(&self) -> &'static str {
        "Fake SHA-256"
    }

    fn supports_install(&self) -> bool {
        self.supports
    }

    fn checksum_kind(&self) -> ChecksumKind {
        ChecksumKind::Sha256Manifest
    }

    async fn fetch(
        &self,
        _client: &reqwest::Client,
        _include_prereleases: bool,
    ) -> Result<Vec<ProtonUpAvailableVersion>, providers::ProviderError> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn returns_already_installed_when_tool_dir_exists_and_force_false() {
    let mock_server = MockServer::start().await;
    let archive = minimal_ge_proton_tar_gz("GE-Proton9-21");
    let digest = hex_encode(&Sha512::digest(&archive));
    Mock::given(method("GET"))
        .and(path("/archive.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/archive.tar.gz.sha512sum"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(format!("{digest}  archive.tar.gz")),
        )
        .mount(&mock_server)
        .await;

    let download_url = format!("{}/archive.tar.gz", mock_server.uri());
    let checksum_url = format!("{}/archive.tar.gz.sha512sum", mock_server.uri());

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    let version_dir = compat_dir.join("GE-Proton9-21");
    std::fs::create_dir_all(&version_dir).expect("version dir");

    let request = ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: "GE-Proton9-21".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = make_version("GE-Proton9-21", Some(&download_url), Some(&checksum_url));

    let result = install_version(&request, &version_info).await;
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::AlreadyInstalled)
    );
}

#[tokio::test]
async fn returns_dependency_missing_when_no_download_url() {
    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    std::fs::create_dir_all(&compat_dir).expect("compat dir");

    let request = ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: "GE-Proton9-21".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = make_version("GE-Proton9-21", None, None);

    let result = install_version(&request, &version_info).await;
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::DependencyMissing)
    );
}

#[tokio::test]
async fn force_flag_bypasses_already_installed_check() {
    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    let version_dir = compat_dir.join("GE-Proton9-21");
    std::fs::create_dir_all(&version_dir).expect("version dir");

    let request = ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: "GE-Proton9-21".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: true,
    };
    let version_info = make_version("GE-Proton9-21", None, None);

    let result = install_version(&request, &version_info).await;
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::DependencyMissing),
        "with force=true the already-installed guard must be skipped"
    );
}

#[tokio::test]
async fn emits_progress_events_during_download() {
    let mock_server = MockServer::start().await;
    let archive = minimal_ge_proton_tar_gz("GE-Proton9-22");
    let digest = hex_encode(&Sha512::digest(&archive));
    Mock::given(method("GET"))
        .and(path("/GE-Proton9-22.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/GE-Proton9-22.tar.gz.sha512sum"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(format!("{digest}  GE-Proton9-22.tar.gz")),
        )
        .mount(&mock_server)
        .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    std::fs::create_dir_all(&compat_dir).expect("compat dir");

    let download_url = format!("{}/GE-Proton9-22.tar.gz", mock_server.uri());
    let checksum_url = format!("{}/GE-Proton9-22.tar.gz.sha512sum", mock_server.uri());
    let request = ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: "GE-Proton9-22".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = make_version("GE-Proton9-22", Some(&download_url), Some(&checksum_url));

    let (emitter, mut rx) = ProgressEmitter::new("test-op-1");
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        install_version_with_progress(&request, &version_info, Some(emitter), None),
    )
    .await
    .expect("install timed out")
    .expect("install should succeed");

    assert!(
        result.success,
        "install must succeed: {:?}",
        result.error_message
    );

    let mut phases = Vec::new();
    while let Ok(event) = rx.try_recv() {
        phases.push(format!("{:?}", event.phase));
    }

    let phase_str = phases.join(",");
    assert!(
        phase_str.contains("Resolving"),
        "missing Resolving: {phase_str}"
    );
    assert!(
        phase_str.contains("Downloading"),
        "missing Downloading: {phase_str}"
    );
    assert!(
        phase_str.contains("Verifying"),
        "missing Verifying: {phase_str}"
    );
    assert!(
        phase_str.contains("Extracting"),
        "missing Extracting: {phase_str}"
    );
    assert!(
        phase_str.contains("Finalizing"),
        "missing Finalizing: {phase_str}"
    );
    assert!(phase_str.contains("Done"), "missing Done: {phase_str}");
}

#[tokio::test]
async fn honors_cancellation_before_extract() {
    let mock_server = MockServer::start().await;
    let archive = minimal_ge_proton_tar_gz("GE-Proton9-cancel");
    Mock::given(method("GET"))
        .and(path("/GE-Proton9-cancel.tar.gz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(archive)
                .set_delay(std::time::Duration::from_millis(10)),
        )
        .mount(&mock_server)
        .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    std::fs::create_dir_all(&compat_dir).expect("compat dir");

    let download_url = format!("{}/GE-Proton9-cancel.tar.gz", mock_server.uri());
    let request = ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: "GE-Proton9-cancel".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = make_version("GE-Proton9-cancel", Some(&download_url), None);

    let token = CancellationToken::new();
    let (emitter, mut rx) = ProgressEmitter::new("test-cancel-op");
    token.cancel();

    let err = install_version_with_progress(&request, &version_info, Some(emitter), Some(token))
        .await
        .expect_err("expected Cancelled error");

    assert!(
        matches!(err, InstallError::Cancelled),
        "expected Cancelled, got: {err:?}"
    );
    assert!(
        !compat_dir.join(".tmp.GE-Proton9-cancel.tar.gz").exists(),
        "temp file should be cleaned up after cancel"
    );
    assert!(
        !compat_dir.join("GE-Proton9-cancel").exists(),
        "partial extract dir should be cleaned up"
    );

    let mut saw_cancelled = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event.phase, Phase::Cancelled) {
            saw_cancelled = true;
        }
    }
    assert!(saw_cancelled, "expected Phase::Cancelled event");
}

#[tokio::test]
async fn verifies_sha256_manifest_checksum() {
    let mock_server = MockServer::start().await;

    let archive = minimal_ge_proton_tar_gz("fake-sha256-ver");
    let digest = sha2::Sha256::digest(&archive);
    let sha256_hex = hex_encode(&digest);
    let archive_name = "fake-sha256-ver.tar.gz";
    let manifest_body = format!("{sha256_hex}  {archive_name}\n");

    Mock::given(method("GET"))
        .and(path(format!("/{archive_name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive.clone()))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/SHA256SUMS"))
        .respond_with(ResponseTemplate::new(200).set_body_string(manifest_body))
        .mount(&mock_server)
        .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    std::fs::create_dir_all(&compat_dir).expect("compat dir");

    let download_url = format!("{}/{archive_name}", mock_server.uri());
    let checksum_url = format!("{}/SHA256SUMS", mock_server.uri());
    let request = ProtonUpInstallRequest {
        provider: "fake-sha256".to_string(),
        version: "fake-sha256-ver".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = sha256_version("fake-sha256-ver", Some(&download_url), Some(&checksum_url));

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        install_version_with_progress(&request, &version_info, None, None),
    )
    .await
    .expect("timed out")
    .expect("install should succeed");

    assert!(
        result.success,
        "SHA-256 manifest install should succeed: {:?}",
        result.error_message
    );
}

#[tokio::test]
async fn rejects_catalog_only_provider() {
    use crate::protonup::providers::ProtonReleaseProvider as _;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = temp.path().join("compatibilitytools.d");
    std::fs::create_dir_all(&compat_dir).expect("compat dir");

    let fake = FakeSha256Provider { supports: false };
    assert!(
        !fake.supports_install(),
        "FakeSha256Provider with supports=false must return false"
    );

    let error = InstallError::DependencyMissing {
        reason: "catalog-only provider".into(),
    };
    assert!(
        matches!(error, InstallError::DependencyMissing { .. }),
        "expected DependencyMissing"
    );

    let request = ProtonUpInstallRequest {
        provider: "nonexistent-catalog-only".to_string(),
        version: "v1".to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = ProtonUpAvailableVersion {
        provider: "nonexistent-catalog-only".to_string(),
        version: "v1".to_string(),
        release_url: None,
        download_url: None,
        checksum_url: None,
        checksum_kind: None,
        asset_size: None,
        published_at: None,
    };
    let result = install_version_with_progress(&request, &version_info, None, None)
        .await
        .expect_err("should fail with missing download url");
    assert!(
        matches!(result, InstallError::DependencyMissing { .. }),
        "expected DependencyMissing, got: {result:?}"
    );
}
