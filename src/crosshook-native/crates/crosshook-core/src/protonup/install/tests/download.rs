use super::super::{fetch_sha256_manifest, fetch_sha512_sidecar, InstallError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_sha512_sidecar_rejects_non_success_status() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/archive.sha512sum"))
        .respond_with(ResponseTemplate::new(404).set_body_string("missing"))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let checksum_url = format!("{}/archive.sha512sum", mock_server.uri());
    let result = fetch_sha512_sidecar(&client, &checksum_url).await;

    assert!(
        matches!(result, Err(InstallError::NetworkError(_))),
        "expected NetworkError for non-2xx checksum status, got {result:?}"
    );
}

#[tokio::test]
async fn fetch_sha512_sidecar_rejects_streamed_body_over_limit() {
    let mock_server = MockServer::start().await;
    let oversized = "a".repeat(1024 * 1024 + 1);
    Mock::given(method("GET"))
        .and(path("/archive.sha512sum"))
        .respond_with(ResponseTemplate::new(200).set_body_string(oversized))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let checksum_url = format!("{}/archive.sha512sum", mock_server.uri());
    let result = fetch_sha512_sidecar(&client, &checksum_url).await;

    assert!(
        matches!(result, Err(InstallError::ChecksumFailed(_))),
        "expected ChecksumFailed for oversized checksum body, got {result:?}"
    );
}

#[tokio::test]
async fn fetch_sha256_manifest_rejects_non_success_status() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/SHA256SUMS"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let manifest_url = format!("{}/SHA256SUMS", mock_server.uri());
    let result = fetch_sha256_manifest(&client, &manifest_url, "tool.tar.gz").await;

    assert!(
        matches!(result, Err(InstallError::NetworkError(_))),
        "expected NetworkError for non-2xx manifest status, got {result:?}"
    );
}
