use std::path::Path;

use sha2::{Digest, Sha512};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::{hex_encode, install_version_with_progress};
use super::support::{make_version, minimal_ge_proton_tar_gz};
use crate::protonup::{ProtonUpInstallErrorKind, ProtonUpInstallRequest, ProtonUpInstallResult};

/// Mock server on a loopback address that `validate_release_url` does not
/// accept, standing in for a host outside the release allowlist.
async fn start_off_allowlist_server() -> MockServer {
    let listener = std::net::TcpListener::bind("127.0.0.2:0").expect("bind 127.0.0.2");
    MockServer::builder().listener(listener).start().await
}

fn redirect_to(location: &str) -> ResponseTemplate {
    ResponseTemplate::new(302).insert_header("Location", location)
}

async fn mount_get(server: &MockServer, route: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(response)
        .mount(server)
        .await;
}

fn sha512_sidecar(archive: &[u8], archive_name: &str) -> ResponseTemplate {
    let digest = hex_encode(&Sha512::digest(archive));
    ResponseTemplate::new(200).set_body_string(format!("{digest}  {archive_name}"))
}

async fn run_install(
    compat_dir: &Path,
    version: &str,
    download_url: &str,
    checksum_url: &str,
) -> ProtonUpInstallResult {
    let request = ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: version.to_string(),
        target_root: compat_dir.to_string_lossy().to_string(),
        force: false,
    };
    let version_info = make_version(version, Some(download_url), Some(checksum_url));
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        install_version_with_progress(&request, &version_info, None, None)
            .await
            .unwrap_or_else(|error| error.to_result())
    })
    .await
    .expect("install timed out")
}

fn compat_dir_in(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let compat_dir = temp.path().join("compatibilitytools.d");
    std::fs::create_dir_all(&compat_dir).expect("compat dir");
    compat_dir
}

async fn assert_untouched(server: &MockServer) {
    let requests = server.received_requests().await.expect("request recording");
    assert!(
        requests.is_empty(),
        "off-allowlist host must never be contacted, got {} request(s)",
        requests.len()
    );
}

#[tokio::test]
async fn download_redirect_to_off_allowlist_host_is_refused() {
    let version = "GE-Proton9-redir-dl";
    let archive_name = format!("{version}.tar.gz");
    let archive = minimal_ge_proton_tar_gz(version);

    let trusted = MockServer::start().await;
    let untrusted = start_off_allowlist_server().await;
    let archive_route = format!("/{archive_name}");
    let sidecar_route = format!("/{archive_name}.sha512sum");

    mount_get(
        &trusted,
        &archive_route,
        redirect_to(&format!("{}{archive_route}", untrusted.uri())),
    )
    .await;
    mount_get(
        &trusted,
        &sidecar_route,
        sha512_sidecar(&archive, &archive_name),
    )
    .await;
    mount_get(
        &untrusted,
        &archive_route,
        ResponseTemplate::new(200).set_body_bytes(archive),
    )
    .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = compat_dir_in(&temp);
    let result = run_install(
        &compat_dir,
        version,
        &format!("{}{archive_route}", trusted.uri()),
        &format!("{}{sidecar_route}", trusted.uri()),
    )
    .await;

    assert!(!result.success, "install must fail: {result:?}");
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::NetworkError)
    );
    let message = result.error_message.unwrap_or_default();
    assert!(
        !message.contains(&untrusted.uri()),
        "error must not echo the redirect target: {message}"
    );
    assert_untouched(&untrusted).await;
    assert!(!compat_dir.join(format!(".tmp.{archive_name}")).exists());
    assert!(!compat_dir.join(version).exists());
}

/// An `https` hop to a host outside the allowlist reaches the host check
/// (the loopback cases above are refused on scheme) and is refused before
/// any connection is made.
#[tokio::test]
async fn download_redirect_to_off_allowlist_https_host_is_refused() {
    let version = "GE-Proton9-redir-https";
    let archive_name = format!("{version}.tar.gz");
    let archive = minimal_ge_proton_tar_gz(version);

    let trusted = MockServer::start().await;
    let archive_route = format!("/{archive_name}");
    let sidecar_route = format!("/{archive_name}.sha512sum");

    mount_get(
        &trusted,
        &archive_route,
        redirect_to(&format!("https://untrusted.invalid{archive_route}")),
    )
    .await;
    mount_get(
        &trusted,
        &sidecar_route,
        sha512_sidecar(&archive, &archive_name),
    )
    .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = compat_dir_in(&temp);
    let result = run_install(
        &compat_dir,
        version,
        &format!("{}{archive_route}", trusted.uri()),
        &format!("{}{sidecar_route}", trusted.uri()),
    )
    .await;

    assert!(!result.success, "install must fail: {result:?}");
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::NetworkError)
    );
    let message = result.error_message.unwrap_or_default();
    assert!(
        message.contains("redirect to an untrusted host was refused"),
        "the redirect policy must refuse the hop: {message}"
    );
    assert!(!message.contains("untrusted.invalid"));
    assert!(!compat_dir.join(format!(".tmp.{archive_name}")).exists());
    assert!(!compat_dir.join(version).exists());
}

#[tokio::test]
async fn checksum_redirect_to_off_allowlist_host_is_refused() {
    let version = "GE-Proton9-redir-sum";
    let archive_name = format!("{version}.tar.gz");
    let archive = minimal_ge_proton_tar_gz(version);

    let trusted = MockServer::start().await;
    let untrusted = start_off_allowlist_server().await;
    let archive_route = format!("/{archive_name}");
    let sidecar_route = format!("/{archive_name}.sha512sum");

    mount_get(
        &untrusted,
        &sidecar_route,
        sha512_sidecar(&archive, &archive_name),
    )
    .await;
    mount_get(
        &trusted,
        &archive_route,
        ResponseTemplate::new(200).set_body_bytes(archive),
    )
    .await;
    mount_get(
        &trusted,
        &sidecar_route,
        redirect_to(&format!("{}{sidecar_route}", untrusted.uri())),
    )
    .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = compat_dir_in(&temp);
    let result = run_install(
        &compat_dir,
        version,
        &format!("{}{archive_route}", trusted.uri()),
        &format!("{}{sidecar_route}", trusted.uri()),
    )
    .await;

    assert!(!result.success, "install must fail: {result:?}");
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::NetworkError)
    );
    let message = result.error_message.unwrap_or_default();
    assert!(
        !message.contains(&untrusted.uri()),
        "error must not echo the redirect target: {message}"
    );
    assert_untouched(&untrusted).await;
    assert!(!compat_dir.join(format!(".tmp.{archive_name}")).exists());
    assert!(!compat_dir.join(version).exists());
}

#[tokio::test]
async fn redirect_within_allowlist_is_followed() {
    let version = "GE-Proton9-redir-ok";
    let archive_name = format!("{version}.tar.gz");
    let archive = minimal_ge_proton_tar_gz(version);

    let trusted = MockServer::start().await;
    let archive_route = format!("/{archive_name}");
    let sidecar_route = format!("/{archive_name}.sha512sum");

    mount_get(
        &trusted,
        &archive_route,
        redirect_to(&format!("{}/cdn{archive_route}", trusted.uri())),
    )
    .await;
    mount_get(
        &trusted,
        &format!("/cdn{archive_route}"),
        ResponseTemplate::new(200).set_body_bytes(archive.clone()),
    )
    .await;
    mount_get(
        &trusted,
        &sidecar_route,
        redirect_to(&format!("{}/cdn{sidecar_route}", trusted.uri())),
    )
    .await;
    mount_get(
        &trusted,
        &format!("/cdn{sidecar_route}"),
        sha512_sidecar(&archive, &archive_name),
    )
    .await;

    let temp = tempfile::tempdir().expect("temp dir");
    let compat_dir = compat_dir_in(&temp);
    let result = run_install(
        &compat_dir,
        version,
        &format!("{}{archive_route}", trusted.uri()),
        &format!("{}{sidecar_route}", trusted.uri()),
    )
    .await;

    assert!(result.success, "install must succeed: {result:?}");
    assert!(compat_dir.join(version).join("proton").exists());
}
