//! ProtonUp install orchestration with security guardrails.
//!
//! Handles archive download, SHA-512 checksum verification, and extraction to
//! the Steam compatibility tools directory. All paths are validated to prevent
//! writes outside the allowed destinations before any I/O begins.

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha512};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::protonup::{
    ProtonUpAvailableVersion, ProtonUpInstallErrorKind, ProtonUpInstallRequest,
    ProtonUpInstallResult,
};
use crate::settings::expand_path_with_tilde;

// ── path validation ───────────────────────────────────────────────────────────

/// Validate that `target_root` is a legitimate Steam compatibility tools
/// directory and return the canonicalized path.
///
/// Allowed destinations:
/// - Must end with `compatibilitytools.d` or be a child of it.
/// - Must not contain `..` path components.
/// - Must not be empty.
/// - If the directory does not yet exist it is created (including parents).
fn validate_install_destination(target_root: &str) -> Result<PathBuf, ProtonUpInstallResult> {
    let raw = target_root.trim();
    if raw.is_empty() {
        return Err(err(
            "install destination path is empty",
            ProtonUpInstallErrorKind::InvalidPath,
        ));
    }

    // Expand ~ to the user's home directory so frontend paths like
    // `~/.local/share/Steam/compatibilitytools.d` resolve correctly.
    let path = if raw.starts_with('~') {
        expand_path_with_tilde(raw).map_err(|e| {
            err(
                format!("failed to expand path: {e}"),
                ProtonUpInstallErrorKind::InvalidPath,
            )
        })?
    } else {
        PathBuf::from(raw)
    };

    // Reject any path that contains a `..` component.
    for component in path.components() {
        if component == Component::ParentDir {
            return Err(err(
                "install destination path contains '..' components",
                ProtonUpInstallErrorKind::InvalidPath,
            ));
        }
    }

    // Require that `compatibilitytools.d` appears somewhere in the path.
    let has_compat_segment = path.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("compatibilitytools.d")
    });
    if !has_compat_segment {
        return Err(err(
            "install destination must be under a 'compatibilitytools.d' directory",
            ProtonUpInstallErrorKind::InvalidPath,
        ));
    }

    // Create the directory (with parents) if it does not exist.
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|io_err| {
            if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                err(
                    format!("permission denied creating {}: {io_err}", path.display()),
                    ProtonUpInstallErrorKind::PermissionDenied,
                )
            } else {
                err(
                    format!("failed to create {}: {io_err}", path.display()),
                    ProtonUpInstallErrorKind::Unknown,
                )
            }
        })?;
    }

    // Canonicalize after ensuring the directory exists.
    let canonical = path.canonicalize().map_err(|io_err| {
        err(
            format!("failed to resolve {}: {io_err}", path.display()),
            ProtonUpInstallErrorKind::InvalidPath,
        )
    })?;

    Ok(canonical)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn err(message: impl Into<String>, kind: ProtonUpInstallErrorKind) -> ProtonUpInstallResult {
    ProtonUpInstallResult {
        success: false,
        installed_path: None,
        error_kind: Some(kind),
        error_message: Some(message.into()),
    }
}

fn network_err(message: impl std::fmt::Display) -> ProtonUpInstallResult {
    err(message.to_string(), ProtonUpInstallErrorKind::NetworkError)
}

fn permission_err(message: impl std::fmt::Display) -> ProtonUpInstallResult {
    err(
        message.to_string(),
        ProtonUpInstallErrorKind::PermissionDenied,
    )
}

fn unknown_err(message: impl std::fmt::Display) -> ProtonUpInstallResult {
    err(message.to_string(), ProtonUpInstallErrorKind::Unknown)
}

/// Map an `io::Error` to the appropriate `ProtonUpInstallResult` failure.
fn map_io_err(io_err: std::io::Error, context: &str) -> ProtonUpInstallResult {
    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
        permission_err(format!("{context}: {io_err}"))
    } else {
        unknown_err(format!("{context}: {io_err}"))
    }
}

// ── download helpers ──────────────────────────────────────────────────────────

/// Stream the URL into `dest_path`, returning the SHA-512 digest of the bytes
/// written.
async fn download_to_file(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
) -> Result<Vec<u8>, ProtonUpInstallResult> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| network_err(format!("request failed for {url}: {e}")))?;

    if !response.status().is_success() {
        return Err(network_err(format!(
            "server returned {} for {url}",
            response.status()
        )));
    }

    let mut file = fs::File::create(dest_path).await.map_err(|e| {
        map_io_err(
            e,
            &format!("failed to create temp file {}", dest_path.display()),
        )
    })?;

    let mut hasher = Sha512::new();
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| network_err(format!("download interrupted: {e}")))?;
        hasher.update(&bytes);
        file.write_all(&bytes)
            .await
            .map_err(|e| map_io_err(e, &format!("write failed to {}", dest_path.display())))?;
    }

    file.flush()
        .await
        .map_err(|e| map_io_err(e, &format!("flush failed for {}", dest_path.display())))?;

    Ok(hasher.finalize().to_vec())
}

/// Fetch and parse a `.sha512sum` file, returning the hex hash string.
///
/// Expected format: `<hex-hash>  <filename>` — one line, two-space separator.
async fn fetch_expected_checksum(
    client: &reqwest::Client,
    checksum_url: &str,
) -> Result<String, ProtonUpInstallResult> {
    let body = client
        .get(checksum_url)
        .send()
        .await
        .map_err(|e| network_err(format!("checksum request failed for {checksum_url}: {e}")))?
        .text()
        .await
        .map_err(|e| network_err(format!("failed to read checksum body: {e}")))?;

    // Format: `<hash>  <filename>`
    let hash = body
        .lines()
        .find_map(|line| {
            // Split on two-space separator.
            let (hash_part, _rest) = line.split_once("  ")?;
            let trimmed = hash_part.trim();
            if trimmed.len() == 128 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(trimmed.to_lowercase())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            err(
                format!("could not parse SHA-512 hash from checksum file at {checksum_url}"),
                ProtonUpInstallErrorKind::ChecksumFailed,
            )
        })?;

    Ok(hash)
}

// ── extract helper ────────────────────────────────────────────────────────────

/// Extract a `.tar.gz` or `.tar.xz` stream to `dest_dir` and return the name of the first
/// top-level directory extracted (the tool directory).
///
/// Blocking I/O runs in `spawn_blocking` via [`extract_archive`] / [`peek_archive`].

fn extract_tar_read_sync<R: std::io::Read>(
    read: R,
    dest_dir: &Path,
) -> Result<String, ProtonUpInstallResult> {
    use tar::Archive;

    let mut archive = Archive::new(read);

    let mut top_level_dir: Option<String> = None;

    let entries = archive
        .entries()
        .map_err(|e| unknown_err(format!("failed to read archive entries: {e}")))?;

    for entry_result in entries {
        let mut entry =
            entry_result.map_err(|e| unknown_err(format!("failed to read archive entry: {e}")))?;

        let entry_path = entry
            .path()
            .map_err(|e| unknown_err(format!("invalid path in archive entry: {e}")))?;

        // Capture the top-level directory name from the first component.
        if top_level_dir.is_none() {
            if let Some(first) = entry_path.components().next() {
                let name = first.as_os_str().to_string_lossy().to_string();
                if !name.is_empty() && name != "." {
                    top_level_dir = Some(name);
                }
            }
        }

        entry.unpack_in(dest_dir).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                permission_err(format!(
                    "permission denied extracting to {}: {e}",
                    dest_dir.display()
                ))
            } else {
                unknown_err(format!("extraction error: {e}"))
            }
        })?;
    }

    top_level_dir.ok_or_else(|| unknown_err("archive appears to be empty"))
}

fn extract_tar_gz_sync(
    archive_path: &Path,
    dest_dir: &Path,
) -> Result<String, ProtonUpInstallResult> {
    use flate2::read::GzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;

    let gz = GzDecoder::new(file);
    extract_tar_read_sync(gz, dest_dir)
}

fn extract_tar_xz_sync(
    archive_path: &Path,
    dest_dir: &Path,
) -> Result<String, ProtonUpInstallResult> {
    use xz2::read::XzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;

    let xz = XzDecoder::new(file);
    extract_tar_read_sync(xz, dest_dir)
}

/// Read the first top-level directory name from a tar stream without extracting.
///
/// Used so install-target paths match what `archive_extract_sync` will create.
fn peek_tar_read_top_level_sync<R: std::io::Read>(
    read: R,
) -> Result<String, ProtonUpInstallResult> {
    use tar::Archive;

    let mut archive = Archive::new(read);

    let entries = archive
        .entries()
        .map_err(|e| unknown_err(format!("failed to read archive entries: {e}")))?;

    for entry_result in entries {
        let entry =
            entry_result.map_err(|e| unknown_err(format!("failed to read archive entry: {e}")))?;

        let entry_path = entry
            .path()
            .map_err(|e| unknown_err(format!("invalid path in archive entry: {e}")))?;

        if let Some(first) = entry_path.components().next() {
            let name = first.as_os_str().to_string_lossy().to_string();
            if !name.is_empty() && name != "." {
                return Ok(name);
            }
        }
    }

    Err(unknown_err("archive appears to be empty"))
}

fn peek_tar_gz_top_level_sync(archive_path: &Path) -> Result<String, ProtonUpInstallResult> {
    use flate2::read::GzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;

    let gz = GzDecoder::new(file);
    peek_tar_read_top_level_sync(gz)
}

fn peek_tar_xz_top_level_sync(archive_path: &Path) -> Result<String, ProtonUpInstallResult> {
    use xz2::read::XzDecoder;

    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;

    let xz = XzDecoder::new(file);
    peek_tar_read_top_level_sync(xz)
}

fn archive_peek_sync(archive_path: &Path) -> Result<String, ProtonUpInstallResult> {
    let name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.ends_with(".tar.xz") {
        peek_tar_xz_top_level_sync(archive_path)
    } else if name.ends_with(".tar.gz") {
        peek_tar_gz_top_level_sync(archive_path)
    } else {
        Err(unknown_err(format!(
            "unsupported archive format (expected .tar.gz or .tar.xz): {name}"
        )))
    }
}

fn archive_extract_sync(
    archive_path: &Path,
    dest_dir: &Path,
) -> Result<String, ProtonUpInstallResult> {
    let name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.ends_with(".tar.xz") {
        extract_tar_xz_sync(archive_path, dest_dir)
    } else if name.ends_with(".tar.gz") {
        extract_tar_gz_sync(archive_path, dest_dir)
    } else {
        Err(unknown_err(format!(
            "unsupported archive format (expected .tar.gz or .tar.xz): {name}"
        )))
    }
}

async fn peek_archive(archive_path: PathBuf) -> Result<String, ProtonUpInstallResult> {
    tokio::task::spawn_blocking(move || archive_peek_sync(&archive_path))
        .await
        .map_err(|e| unknown_err(format!("peek task panicked: {e}")))?
}

async fn extract_archive(
    archive_path: PathBuf,
    dest_dir: PathBuf,
) -> Result<String, ProtonUpInstallResult> {
    tokio::task::spawn_blocking(move || archive_extract_sync(&archive_path, &dest_dir))
        .await
        .map_err(|e| unknown_err(format!("extraction task panicked: {e}")))?
}

// ── public install entry point ────────────────────────────────────────────────

/// Execute a Proton version install.
///
/// Steps:
/// 1. Validate install destination path.
/// 2. Resolve download URL.
/// 3. Download the archive to a temporary file.
/// 4. Download and verify the SHA-512 checksum.
/// 5. Peek the archive top-level directory name. For GE-Proton this must match
///    `version_info.version`; Proton-CachyOS uses release tags that differ from the
///    per-arch folder name (e.g. tag `cachyos-…-slr` vs `proton-cachyos-…-x86_64`).
/// 6. If not `force`, return [`ProtonUpInstallErrorKind::AlreadyInstalled`] when that
///    directory already exists under `dest_dir`.
/// 7. Extract the archive to the destination directory.
/// 8. Verify the extracted directory contains a `proton` executable.
/// 9. Clean up the temporary archive file.
pub async fn install_version(
    request: &ProtonUpInstallRequest,
    version_info: &ProtonUpAvailableVersion,
) -> ProtonUpInstallResult {
    // 1. Validate destination path.
    let dest_dir = match validate_install_destination(&request.target_root) {
        Ok(path) => path,
        Err(result) => return result,
    };

    // 2. Resolve download URL.
    let download_url = match &version_info.download_url {
        Some(url) => url.clone(),
        None => {
            return err(
                format!(
                    "no download URL available for version {}",
                    version_info.version
                ),
                ProtonUpInstallErrorKind::DependencyMissing,
            )
        }
    };

    // Derive a temp filename from the URL's last path segment.
    let archive_filename = download_url
        .rsplit('/')
        .next()
        .unwrap_or("proton-archive.tar.gz")
        .to_string();
    let temp_path = dest_dir.join(format!(".tmp.{archive_filename}"));

    let client = reqwest::Client::new();

    // 3. Download archive, capturing the SHA-512 of the bytes written.
    let actual_digest = match download_to_file(&client, &download_url, &temp_path).await {
        Ok(digest) => digest,
        Err(result) => {
            let _ = std::fs::remove_file(&temp_path);
            return result;
        }
    };

    // 4. Verify checksum if a checksum URL is available.
    if let Some(checksum_url) = &version_info.checksum_url {
        let expected_hex = match fetch_expected_checksum(&client, checksum_url).await {
            Ok(hex) => hex,
            Err(result) => {
                let _ = std::fs::remove_file(&temp_path);
                return result;
            }
        };

        let actual_hex = hex_encode(&actual_digest);
        if actual_hex != expected_hex {
            let _ = std::fs::remove_file(&temp_path);
            return err(
                format!(
                    "SHA-512 checksum mismatch for {}: expected {expected_hex}, got {actual_hex}",
                    version_info.version
                ),
                ProtonUpInstallErrorKind::ChecksumFailed,
            );
        }

        tracing::info!(
            version = %version_info.version,
            "SHA-512 checksum verified"
        );
    } else {
        tracing::warn!(
            version = %version_info.version,
            "no checksum URL available; skipping checksum verification"
        );
    }

    // 5. Discover install target from the archive (same top-level dir as extraction).
    let top_level_dir = match peek_archive(temp_path.clone()).await {
        Ok(dir_name) => dir_name,
        Err(result) => {
            let _ = std::fs::remove_file(&temp_path);
            return result;
        }
    };

    // GE-Proton (and similar) archives use the release tag as the root folder name.
    // Proton-CachyOS tags do not: the tarball unpacks to `proton-cachyos-…-<arch>`.
    if version_info.provider != "proton-cachyos" && top_level_dir != version_info.version {
        let _ = std::fs::remove_file(&temp_path);
        return err(
            format!(
                "archive top-level directory '{top_level_dir}' does not match expected version '{}'",
                version_info.version
            ),
            ProtonUpInstallErrorKind::InvalidPath,
        );
    }

    let installed_dir = dest_dir.join(&top_level_dir);
    if !request.force && installed_dir.exists() {
        let _ = std::fs::remove_file(&temp_path);
        return ProtonUpInstallResult {
            success: false,
            installed_path: Some(installed_dir.to_string_lossy().to_string()),
            error_kind: Some(ProtonUpInstallErrorKind::AlreadyInstalled),
            error_message: Some(format!(
                "version {} is already installed at {}",
                version_info.version,
                installed_dir.display()
            )),
        };
    }

    // 6. Extract archive.
    let extracted_top = match extract_archive(temp_path.clone(), dest_dir.clone()).await {
        Ok(dir_name) => dir_name,
        Err(result) => {
            let _ = std::fs::remove_file(&temp_path);
            return result;
        }
    };

    if extracted_top != top_level_dir {
        let _ = std::fs::remove_file(&temp_path);
        return err(
            format!(
                "archive top-level directory changed between peek and extract (peek: {top_level_dir}, extract: {extracted_top})"
            ),
            ProtonUpInstallErrorKind::Unknown,
        );
    }

    // 7. Clean up the temp archive.
    if let Err(e) = fs::remove_file(&temp_path).await {
        tracing::warn!(
            path = %temp_path.display(),
            error = %e,
            "failed to remove temp archive after extraction"
        );
    }

    // 8. Verify extracted directory contains a `proton` executable.
    let proton_bin = installed_dir.join("proton");
    if !proton_bin.is_file() {
        // Attempt cleanup of the partial extraction.
        let _ = std::fs::remove_dir_all(&installed_dir);
        return err(
            format!(
                "extracted archive does not contain a 'proton' executable at {}",
                proton_bin.display()
            ),
            ProtonUpInstallErrorKind::Unknown,
        );
    }

    tracing::info!(
        version = %version_info.version,
        path = %installed_dir.display(),
        "Proton version installed successfully"
    );

    ProtonUpInstallResult {
        success: true,
        installed_path: Some(installed_dir.to_string_lossy().to_string()),
        error_kind: None,
        error_message: None,
    }
}

// ── utilities ─────────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(128), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn minimal_ge_proton_tar_gz(tool_dir_name: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let enc = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
            let mut builder = tar::Builder::new(enc);
            let mut header = tar::Header::new_gnu();
            header
                .set_path(format!("{tool_dir_name}/proton"))
                .expect("tar path");
            header.set_size(0);
            header.set_cksum();
            builder
                .append(&header, &mut std::io::empty())
                .expect("append empty proton");
            builder.finish().expect("tar finish");
        }
        buf
    }

    fn make_version(
        version: &str,
        download_url: Option<&str>,
        checksum_url: Option<&str>,
    ) -> ProtonUpAvailableVersion {
        ProtonUpAvailableVersion {
            provider: "ge-proton".to_string(),
            version: version.to_string(),
            release_url: None,
            download_url: download_url.map(str::to_string),
            checksum_url: checksum_url.map(str::to_string),
            checksum_kind: Some("sha512".to_string()),
            asset_size: None,
        }
    }

    // ── validate_install_destination ─────────────────────────────────────────

    #[test]
    fn rejects_empty_path() {
        let result = validate_install_destination("").unwrap_err();
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::InvalidPath)
        );
        assert!(result.error_message.unwrap().contains("empty"));
    }

    #[test]
    fn rejects_path_with_parent_dir_component() {
        let result = validate_install_destination(
            "/home/user/.steam/../../../etc/passwd/compatibilitytools.d",
        )
        .unwrap_err();
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::InvalidPath)
        );
    }

    #[test]
    fn rejects_path_without_compatibilitytools_d_segment() {
        let result = validate_install_destination("/home/user/.steam/root/steamapps").unwrap_err();
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::InvalidPath)
        );
        assert!(result
            .error_message
            .unwrap()
            .contains("compatibilitytools.d"));
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

    // ── hex_encode ────────────────────────────────────────────────────────────

    #[test]
    fn hex_encode_produces_lowercase_hex() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        assert_eq!(hex_encode(&bytes), "deadbeef");
    }

    // ── already_installed check ───────────────────────────────────────────────

    #[tokio::test]
    async fn returns_already_installed_when_tool_dir_exists_and_force_false() {
        let mock_server = MockServer::start().await;
        let archive = minimal_ge_proton_tar_gz("GE-Proton9-21");
        Mock::given(method("GET"))
            .and(path("/archive.tar.gz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
            .mount(&mock_server)
            .await;

        let download_url = format!("{}/archive.tar.gz", mock_server.uri());

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
        let version_info = make_version("GE-Proton9-21", Some(&download_url), None);

        let result = install_version(&request, &version_info).await;
        assert!(!result.success);
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::AlreadyInstalled)
        );
    }

    // ── missing download URL ──────────────────────────────────────────────────

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

    // ── force flag bypasses already-installed check ───────────────────────────

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
        // No download URL — install will fail on the dependency-missing check,
        // which proves that AlreadyInstalled was NOT returned.
        let version_info = make_version("GE-Proton9-21", None, None);

        let result = install_version(&request, &version_info).await;
        assert!(!result.success);
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::DependencyMissing),
            "with force=true the already-installed guard must be skipped"
        );
    }

    // ── error_kind construction ───────────────────────────────────────────────

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
    fn network_err_sets_network_error_kind() {
        let result = network_err("connection refused");
        assert!(!result.success);
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::NetworkError)
        );
    }

    #[test]
    fn permission_err_sets_permission_denied_kind() {
        let result = permission_err("access denied");
        assert!(!result.success);
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::PermissionDenied)
        );
    }

    #[test]
    fn unknown_err_sets_unknown_kind() {
        let result = unknown_err("something went wrong");
        assert!(!result.success);
        assert_eq!(result.error_kind, Some(ProtonUpInstallErrorKind::Unknown));
    }

    // ── hex_encode ─────────────────────────────────────────────────────────────

    #[test]
    fn hex_encode_empty_bytes_gives_empty_string() {
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn hex_encode_single_byte() {
        assert_eq!(hex_encode(&[0xff]), "ff");
        assert_eq!(hex_encode(&[0x00]), "00");
    }
}
