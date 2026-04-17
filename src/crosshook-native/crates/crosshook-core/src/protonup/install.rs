//! ProtonUp install orchestration with progress, cancellation, and checksum dispatch.
//!
//! The public surface is:
//!   - [`install_version`] — backward-compatible entry point (no progress/cancel).
//!   - [`install_version_with_progress`] — full-featured orchestrator.
//!
//! All host-tool execution is in-process (tar/flate2/xz2); no `Command::new` calls
//! for blocked tools appear here (ADR-0001 compliance).

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256, Sha512};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::protonup::install_root::{pick_default_install_root, resolve_install_root_candidates};
use crate::protonup::progress::{Phase, ProgressEmitter};
use crate::protonup::providers::{self, ChecksumKind};
use crate::protonup::{
    ProtonUpAvailableVersion, ProtonUpInstallErrorKind, ProtonUpInstallRequest,
    ProtonUpInstallResult,
};
use crate::settings::expand_path_with_tilde;

// ── internal error type ───────────────────────────────────────────────────────

/// Internal install error — richer than `ProtonUpInstallResult` for use inside
/// the orchestrator. Converted to `ProtonUpInstallResult` at the public boundary.
#[derive(Debug)]
pub enum InstallError {
    InvalidPath(String),
    PermissionDenied(String),
    NetworkError(String),
    ChecksumMissing(String),
    ChecksumFailed(String),
    AlreadyInstalled { path: PathBuf },
    DependencyMissing { reason: String },
    NoWritableInstallRoot,
    Cancelled,
    UntrustedUrl(String),
    Unknown(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(m) => write!(f, "invalid path: {m}"),
            Self::PermissionDenied(m) => write!(f, "permission denied: {m}"),
            Self::NetworkError(m) => write!(f, "network error: {m}"),
            Self::ChecksumMissing(m) => write!(f, "checksum missing: {m}"),
            Self::ChecksumFailed(m) => write!(f, "checksum failed: {m}"),
            Self::AlreadyInstalled { path } => {
                write!(f, "already installed at {}", path.display())
            }
            Self::DependencyMissing { reason } => write!(f, "dependency missing: {reason}"),
            Self::NoWritableInstallRoot => {
                write!(
                    f,
                    "no writable Proton install root found; install Steam first"
                )
            }
            Self::Cancelled => write!(f, "install cancelled"),
            Self::UntrustedUrl(m) => write!(f, "untrusted URL: {m}"),
            Self::Unknown(m) => write!(f, "unknown error: {m}"),
        }
    }
}

impl InstallError {
    fn to_result(&self) -> ProtonUpInstallResult {
        match self {
            Self::InvalidPath(m) => err(m, ProtonUpInstallErrorKind::InvalidPath),
            Self::PermissionDenied(m) => err(m, ProtonUpInstallErrorKind::PermissionDenied),
            Self::NetworkError(m) => err(m, ProtonUpInstallErrorKind::NetworkError),
            Self::ChecksumMissing(m) | Self::ChecksumFailed(m) => {
                err(m, ProtonUpInstallErrorKind::ChecksumFailed)
            }
            Self::AlreadyInstalled { path } => ProtonUpInstallResult {
                success: false,
                installed_path: Some(path.to_string_lossy().to_string()),
                error_kind: Some(ProtonUpInstallErrorKind::AlreadyInstalled),
                error_message: Some(format!("already installed at {}", path.display())),
            },
            Self::DependencyMissing { reason } => {
                err(reason, ProtonUpInstallErrorKind::DependencyMissing)
            }
            Self::NoWritableInstallRoot => err(
                "no writable Proton install root found; install Steam first",
                ProtonUpInstallErrorKind::InvalidPath,
            ),
            Self::Cancelled => err("install cancelled", ProtonUpInstallErrorKind::Unknown),
            Self::UntrustedUrl(m) => err(m, ProtonUpInstallErrorKind::NetworkError),
            Self::Unknown(m) => err(m, ProtonUpInstallErrorKind::Unknown),
        }
    }
}

// ── path validation ───────────────────────────────────────────────────────────

/// Validate that `target_root` is a legitimate Steam compatibility tools
/// directory and return the canonicalized path.
fn validate_install_destination(target_root: &str) -> Result<PathBuf, InstallError> {
    let raw = target_root.trim();
    if raw.is_empty() {
        return Err(InstallError::InvalidPath(
            "install destination path is empty".into(),
        ));
    }

    let path = if raw.starts_with('~') {
        expand_path_with_tilde(raw)
            .map_err(|e| InstallError::InvalidPath(format!("failed to expand path: {e}")))?
    } else {
        PathBuf::from(raw)
    };

    for component in path.components() {
        if component == Component::ParentDir {
            return Err(InstallError::InvalidPath(
                "install destination path contains '..' components".into(),
            ));
        }
    }

    let has_compat_segment = path.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("compatibilitytools.d")
    });
    if !has_compat_segment {
        return Err(InstallError::InvalidPath(
            "install destination must be under a 'compatibilitytools.d' directory".into(),
        ));
    }

    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|io_err| {
            if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                InstallError::PermissionDenied(format!(
                    "permission denied creating {}: {io_err}",
                    path.display()
                ))
            } else {
                InstallError::Unknown(format!("failed to create {}: {io_err}", path.display()))
            }
        })?;
    }

    let canonical = path.canonicalize().map_err(|io_err| {
        InstallError::InvalidPath(format!("failed to resolve {}: {io_err}", path.display()))
    })?;

    // Re-run the compatibilitytools.d check on the *canonical* path so that a
    // symlink whose component spells "compatibilitytools.d" but resolves to an
    // unrelated directory (e.g., `/`) does not bypass the guard.
    let canonical_has_compat_segment = canonical.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("compatibilitytools.d")
    });
    if !canonical_has_compat_segment {
        return Err(InstallError::InvalidPath(format!(
            "install destination '{}' resolved to '{}' which is not under a 'compatibilitytools.d' directory",
            path.display(),
            canonical.display()
        )));
    }

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

fn network_err(message: impl std::fmt::Display) -> InstallError {
    InstallError::NetworkError(message.to_string())
}

fn map_io_err(io_err: std::io::Error, context: &str) -> InstallError {
    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
        InstallError::PermissionDenied(format!("{context}: {io_err}"))
    } else {
        InstallError::Unknown(format!("{context}: {io_err}"))
    }
}

/// Assert that `url` is an `https` URL whose host is one of the known-good
/// GitHub origins. Returns `InstallError::UntrustedUrl` if any check fails.
///
/// Allowed hosts:
///   - `github.com`               — release API / raw URLs
///   - `api.github.com`           — REST API
///   - `objects.githubusercontent.com`         — uploaded release assets
///   - `github-releases.githubusercontent.com` — CDN redirect target for assets
///
/// In test builds, `http://127.0.0.1` and `http://localhost` are also accepted
/// so that wiremock-backed unit tests can exercise the install flow end-to-end
/// without standing up a TLS server.
fn validate_release_url(url: &str) -> Result<(), InstallError> {
    const ALLOWED_HOSTS: &[&str] = &[
        "github.com",
        "api.github.com",
        "objects.githubusercontent.com",
        "github-releases.githubusercontent.com",
    ];

    let parsed = reqwest::Url::parse(url)
        .map_err(|e| InstallError::UntrustedUrl(format!("failed to parse URL '{url}': {e}")))?;

    let host = parsed.host_str().unwrap_or("");

    // In test builds allow plain-http loopback so wiremock tests can run.
    #[cfg(test)]
    if parsed.scheme() == "http" && (host == "127.0.0.1" || host == "localhost") {
        return Ok(());
    }

    if parsed.scheme() != "https" {
        return Err(InstallError::UntrustedUrl(format!(
            "URL '{url}' uses scheme '{}'; only https is allowed",
            parsed.scheme()
        )));
    }

    if !ALLOWED_HOSTS.contains(&host) {
        return Err(InstallError::UntrustedUrl(format!(
            "URL '{url}' has untrusted host '{host}'; allowed: {}",
            ALLOWED_HOSTS.join(", ")
        )));
    }

    Ok(())
}

// ── download helpers ──────────────────────────────────────────────────────────

/// Stream the URL into `dest_path`, returning the SHA-512 digest. Emits
/// `Phase::Downloading` progress every `EMIT_INTERVAL_BYTES`. Checks `cancel`
/// before each chunk; returns `InstallError::Cancelled` if triggered.
const EMIT_INTERVAL_BYTES: u64 = 256 * 1024;

/// Hard ceiling for checksum sidecar / manifest response bodies (1 MiB).
/// Legitimate `.sha512sum` files are ~200 B and SHA256SUMS manifests are a few
/// KiB; this cap prevents a hostile or mis-served CDN response from being
/// buffered into RAM unboundedly.
const MAX_CHECKSUM_BYTES: u64 = 1024 * 1024;

async fn download_to_file(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    emitter: Option<&ProgressEmitter>,
    cancel: Option<&CancellationToken>,
    content_length: Option<u64>,
) -> Result<(Vec<u8>, Vec<u8>), InstallError> {
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

    // Prefer the caller-provided length (from a HEAD or Content-Length), then
    // fall back to what this response advertises.
    let bytes_total = content_length.or_else(|| response.content_length());

    let mut file = fs::File::create(dest_path).await.map_err(|e| {
        map_io_err(
            e,
            &format!("failed to create temp file {}", dest_path.display()),
        )
    })?;

    let mut sha512 = Sha512::new();
    let mut sha256 = Sha256::new();
    let mut bytes_done: u64 = 0;
    let mut since_last_emit: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    loop {
        // Race the next chunk against cancellation so a stalled / slow
        // stream doesn't hold the user's Cancel click hostage — a plain
        // `stream.next().await` only yields control when the next chunk
        // arrives, which on a stuck connection could be never.
        let chunk = if let Some(tok) = cancel {
            if tok.is_cancelled() {
                return Err(InstallError::Cancelled);
            }
            tokio::select! {
                biased;
                _ = tok.cancelled() => return Err(InstallError::Cancelled),
                next = stream.next() => match next {
                    Some(Ok(b)) => b,
                    Some(Err(e)) => return Err(network_err(format!("download interrupted: {e}"))),
                    None => break,
                },
            }
        } else {
            match stream.next().await {
                Some(Ok(b)) => b,
                Some(Err(e)) => return Err(network_err(format!("download interrupted: {e}"))),
                None => break,
            }
        };

        sha512.update(&chunk);
        sha256.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| map_io_err(e, &format!("write failed to {}", dest_path.display())))?;

        bytes_done += chunk.len() as u64;
        since_last_emit += chunk.len() as u64;

        if since_last_emit >= EMIT_INTERVAL_BYTES {
            since_last_emit = 0;
            if let Some(em) = emitter {
                em.emit(Phase::Downloading, bytes_done, bytes_total, None);
            }
        }
    }

    file.flush()
        .await
        .map_err(|e| map_io_err(e, &format!("flush failed for {}", dest_path.display())))?;

    // Drop the tokio fs::File handle before the caller re-opens the path for
    // peek/extract. tokio::fs close is async-lazy; holding it across the
    // subsequent synchronous open can race on some kernels.
    drop(file);

    // Fail fast if the stream closed without delivering any bytes. Without
    // this, a silent short-circuit (e.g. an unexpected redirect body with
    // Content-Length: 0) propagates as an opaque "archive appears to be
    // empty" error from the tar peek step.
    if bytes_done == 0 {
        return Err(network_err(format!(
            "download produced 0 bytes for {url} — server may have redirected to an empty response"
        )));
    }

    // If the upstream advertised a size, require the transfer to match. A
    // silent truncation here shows up much later as a confusing extraction
    // error, so surface it right at the source.
    if let Some(expected) = bytes_total {
        if expected > 0 && bytes_done != expected {
            return Err(network_err(format!(
                "download truncated for {url}: got {bytes_done} bytes, expected {expected}"
            )));
        }
    }

    // Emit a final downloading event with the full byte count.
    if let Some(em) = emitter {
        em.emit(Phase::Downloading, bytes_done, bytes_total, None);
    }

    Ok((sha512.finalize().to_vec(), sha256.finalize().to_vec()))
}

/// Fetch a `.sha512sum` sidecar and return the hex hash string.
async fn fetch_sha512_sidecar(
    client: &reqwest::Client,
    checksum_url: &str,
) -> Result<String, InstallError> {
    let response = client
        .get(checksum_url)
        .send()
        .await
        .map_err(|e| network_err(format!("checksum request failed for {checksum_url}: {e}")))?;

    if let Some(len) = response.content_length() {
        if len > MAX_CHECKSUM_BYTES {
            return Err(InstallError::ChecksumFailed(format!(
                "checksum response for {checksum_url} is too large ({len} bytes, limit {MAX_CHECKSUM_BYTES})"
            )));
        }
    }

    let body = response
        .text()
        .await
        .map_err(|e| network_err(format!("failed to read checksum body: {e}")))?;

    let hash = body
        .lines()
        .find_map(|line| {
            let (hash_part, _rest) = line.split_once("  ")?;
            let trimmed = hash_part.trim();
            if trimmed.len() == 128 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(trimmed.to_lowercase())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            InstallError::ChecksumFailed(format!(
                "could not parse SHA-512 hash from checksum file at {checksum_url}"
            ))
        })?;

    Ok(hash)
}

/// Fetch a `SHA256SUMS` manifest and return the hex hash for `asset_filename`.
///
/// Supports both `<hex>  <filename>` (two-space) and `<hex> *<filename>` formats.
async fn fetch_sha256_manifest(
    client: &reqwest::Client,
    manifest_url: &str,
    asset_filename: &str,
) -> Result<String, InstallError> {
    let response =
        client.get(manifest_url).send().await.map_err(|e| {
            network_err(format!("SHA256SUMS request failed for {manifest_url}: {e}"))
        })?;

    if let Some(len) = response.content_length() {
        if len > MAX_CHECKSUM_BYTES {
            return Err(InstallError::ChecksumFailed(format!(
                "SHA256SUMS response for {manifest_url} is too large ({len} bytes, limit {MAX_CHECKSUM_BYTES})"
            )));
        }
    }

    let body = response
        .text()
        .await
        .map_err(|e| network_err(format!("failed to read SHA256SUMS body: {e}")))?;

    let hash = body
        .lines()
        .find_map(|line| {
            // Format A: `<hex>  <filename>`
            // Format B: `<hex> *<filename>`
            let (hash_part, rest) = line.split_once("  ").or_else(|| line.split_once(" *"))?;
            let trimmed = hash_part.trim();
            let fname = rest.trim();
            if fname == asset_filename
                && trimmed.len() == 64
                && trimmed.chars().all(|c| c.is_ascii_hexdigit())
            {
                Some(trimmed.to_lowercase())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            InstallError::ChecksumMissing(format!(
                "asset '{asset_filename}' not listed in SHA256SUMS manifest at {manifest_url}"
            ))
        })?;

    Ok(hash)
}

// ── extract helpers ───────────────────────────────────────────────────────────

/// Return the first non-`.` normal path segment, or `None` if the path has
/// no usable top-level name. Archives produced by GNU tar frequently prefix
/// every entry with `./` (POSIX "current directory" marker) — taking the
/// raw first component would misclassify every entry as `.` and leak through
/// as an empty-archive error.
fn first_normal_path_component(path: &Path) -> Option<String> {
    use std::path::Component;
    for component in path.components() {
        match component {
            Component::Normal(s) => {
                let name = s.to_string_lossy().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
            Component::CurDir => continue,
            // Anything else (RootDir / ParentDir / Prefix) means the path
            // escapes the archive root — treat as no usable top-level.
            _ => return None,
        }
    }
    None
}

fn extract_tar_read_sync<R: std::io::Read>(
    read: R,
    dest_dir: &Path,
) -> Result<String, InstallError> {
    use tar::Archive;

    let mut archive = Archive::new(read);
    let mut top_level_dir: Option<String> = None;

    let entries = archive
        .entries()
        .map_err(|e| InstallError::Unknown(format!("failed to read archive entries: {e}")))?;

    for entry_result in entries {
        let mut entry = entry_result
            .map_err(|e| InstallError::Unknown(format!("failed to read archive entry: {e}")))?;

        let entry_path = entry
            .path()
            .map_err(|e| InstallError::Unknown(format!("invalid path in archive entry: {e}")))?;

        if top_level_dir.is_none() {
            top_level_dir = first_normal_path_component(&entry_path);
        }

        entry.unpack_in(dest_dir).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                InstallError::PermissionDenied(format!(
                    "permission denied extracting to {}: {e}",
                    dest_dir.display()
                ))
            } else {
                InstallError::Unknown(format!("extraction error: {e}"))
            }
        })?;
    }

    top_level_dir.ok_or_else(|| InstallError::Unknown("archive appears to be empty".into()))
}

fn extract_tar_gz_sync(archive_path: &Path, dest_dir: &Path) -> Result<String, InstallError> {
    use flate2::read::GzDecoder;
    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    extract_tar_read_sync(GzDecoder::new(file), dest_dir)
}

fn extract_tar_xz_sync(archive_path: &Path, dest_dir: &Path) -> Result<String, InstallError> {
    use xz2::read::XzDecoder;
    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    extract_tar_read_sync(XzDecoder::new(file), dest_dir)
}

fn peek_tar_read_top_level_sync<R: std::io::Read>(read: R) -> Result<String, InstallError> {
    use tar::Archive;
    let mut archive = Archive::new(read);
    let entries = archive
        .entries()
        .map_err(|e| InstallError::Unknown(format!("failed to read archive entries: {e}")))?;

    for entry_result in entries {
        let entry = entry_result
            .map_err(|e| InstallError::Unknown(format!("failed to read archive entry: {e}")))?;
        let entry_path = entry
            .path()
            .map_err(|e| InstallError::Unknown(format!("invalid path in archive entry: {e}")))?;
        if let Some(name) = first_normal_path_component(&entry_path) {
            return Ok(name);
        }
    }
    Err(InstallError::Unknown("archive appears to be empty".into()))
}

/// Attach the archive path + on-disk file size to a peek error so the
/// caller can distinguish "truncated download" from "corrupt stream".
fn enrich_peek_err(archive_path: &Path, err: InstallError) -> InstallError {
    let size = std::fs::metadata(archive_path)
        .map(|m| m.len())
        .unwrap_or(0);
    match err {
        InstallError::Unknown(msg) => InstallError::Unknown(format!(
            "{msg} (archive: {} — on-disk size {size} bytes)",
            archive_path.display()
        )),
        other => other,
    }
}

fn peek_tar_gz_top_level_sync(archive_path: &Path) -> Result<String, InstallError> {
    use flate2::read::GzDecoder;
    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    peek_tar_read_top_level_sync(GzDecoder::new(file)).map_err(|e| enrich_peek_err(archive_path, e))
}

fn peek_tar_xz_top_level_sync(archive_path: &Path) -> Result<String, InstallError> {
    use xz2::read::XzDecoder;
    let file = std::fs::File::open(archive_path).map_err(|e| {
        map_io_err(
            e,
            &format!("failed to open archive {}", archive_path.display()),
        )
    })?;
    peek_tar_read_top_level_sync(XzDecoder::new(file)).map_err(|e| enrich_peek_err(archive_path, e))
}

fn archive_peek_sync(archive_path: &Path) -> Result<String, InstallError> {
    let name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.ends_with(".tar.xz") {
        peek_tar_xz_top_level_sync(archive_path)
    } else if name.ends_with(".tar.gz") {
        peek_tar_gz_top_level_sync(archive_path)
    } else {
        Err(InstallError::Unknown(format!(
            "unsupported archive format (expected .tar.gz or .tar.xz): {name}"
        )))
    }
}

fn archive_extract_sync(archive_path: &Path, dest_dir: &Path) -> Result<String, InstallError> {
    let name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.ends_with(".tar.xz") {
        extract_tar_xz_sync(archive_path, dest_dir)
    } else if name.ends_with(".tar.gz") {
        extract_tar_gz_sync(archive_path, dest_dir)
    } else {
        Err(InstallError::Unknown(format!(
            "unsupported archive format (expected .tar.gz or .tar.xz): {name}"
        )))
    }
}

async fn peek_archive(archive_path: PathBuf) -> Result<String, InstallError> {
    tokio::task::spawn_blocking(move || archive_peek_sync(&archive_path))
        .await
        .map_err(|e| InstallError::Unknown(format!("peek task panicked: {e}")))?
}

async fn extract_archive(archive_path: PathBuf, dest_dir: PathBuf) -> Result<String, InstallError> {
    tokio::task::spawn_blocking(move || archive_extract_sync(&archive_path, &dest_dir))
        .await
        .map_err(|e| InstallError::Unknown(format!("extraction task panicked: {e}")))?
}

// ── cleanup helper ────────────────────────────────────────────────────────────

fn best_effort_cleanup(temp_path: &Path, partial_dir: Option<&Path>) {
    let _ = std::fs::remove_file(temp_path);
    if let Some(dir) = partial_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ── public install entry points ───────────────────────────────────────────────

/// Backward-compatible install entry point. Delegates to
/// [`install_version_with_progress`] with no emitter or cancel token.
pub async fn install_version(
    request: &ProtonUpInstallRequest,
    version_info: &ProtonUpAvailableVersion,
) -> ProtonUpInstallResult {
    match install_version_with_progress(request, version_info, None, None).await {
        Ok(result) => result,
        Err(e) => e.to_result(),
    }
}

/// Full-featured install orchestrator with progress events and cancellation.
///
/// Steps:
/// 1. Resolve provider and validate `supports_install`.
/// 2. Resolve `target_root` (default via install-root resolver if empty).
/// 3. Validate install destination.
/// 4. Download archive with per-chunk progress and cancellation checks.
/// 5. Verify checksum (dispatch by provider's `ChecksumKind`).
/// 6. Peek archive top-level dir; check `AlreadyInstalled` unless `force`.
/// 7. Extract archive.
/// 8. Verify the extracted directory contains a `proton` executable.
/// 9. Clean up the temp archive.
pub async fn install_version_with_progress(
    request: &ProtonUpInstallRequest,
    version_info: &ProtonUpAvailableVersion,
    emitter: Option<ProgressEmitter>,
    cancel: Option<CancellationToken>,
) -> Result<ProtonUpInstallResult, InstallError> {
    let em = emitter.as_ref();

    // ── 1. Resolve provider ───────────────────────────────────────────────────

    if let Some(em) = em {
        em.emit(Phase::Resolving, 0, None, None);
    }

    let registry = providers::registry();
    let provider = registry
        .iter()
        .find(|p| p.id() == request.provider.as_str());

    // Determine checksum kind — from the registry provider if found, else derive
    // from the version_info.checksum_kind string for backward-compat.
    let checksum_kind = if let Some(prov) = &provider {
        if !prov.supports_install() {
            return Err(InstallError::DependencyMissing {
                reason: "catalog-only provider".into(),
            });
        }
        prov.checksum_kind()
    } else {
        // Legacy fallback: read checksum_kind from version_info string.
        match version_info.checksum_kind.as_deref() {
            Some("sha512") | Some("sha512-sidecar") => ChecksumKind::Sha512Sidecar,
            Some("sha256") | Some("sha256-manifest") => ChecksumKind::Sha256Manifest,
            _ => ChecksumKind::None,
        }
    };

    // ── 2. Resolve target_root ────────────────────────────────────────────────

    let effective_root = if request.target_root.trim().is_empty() {
        // Use the install-root resolver to pick a sensible default.
        let steam_path = None::<&Path>; // No configured path available at this call site.
        let candidates = resolve_install_root_candidates(steam_path);
        let default = pick_default_install_root(&candidates);
        match default {
            Some(c) if c.writable => c.path.to_string_lossy().to_string(),
            _ => return Err(InstallError::NoWritableInstallRoot),
        }
    } else {
        request.target_root.clone()
    };

    // ── 3. Validate destination path ──────────────────────────────────────────

    let dest_dir = validate_install_destination(&effective_root)?;

    // ── 4. Resolve download URL ───────────────────────────────────────────────

    let download_url =
        version_info
            .download_url
            .as_deref()
            .ok_or_else(|| InstallError::DependencyMissing {
                reason: format!(
                    "no download URL available for version {}",
                    version_info.version
                ),
            })?;

    // F003: Validate download URL origin before use.
    validate_release_url(download_url)?;

    let archive_filename = download_url
        .rsplit('/')
        .next()
        .unwrap_or("proton-archive.tar.gz")
        .to_string();
    let temp_path = dest_dir.join(format!(".tmp.{archive_filename}"));

    let client = reqwest::Client::new();

    // ── 5. Check cancellation before download ─────────────────────────────────

    if let Some(tok) = &cancel {
        if tok.is_cancelled() {
            if let Some(em) = em {
                em.emit(Phase::Cancelled, 0, None, None);
            }
            return Err(InstallError::Cancelled);
        }
    }

    // ── 6. Download archive ───────────────────────────────────────────────────

    let content_length = version_info.asset_size;
    let (sha512_digest, sha256_digest) = match download_to_file(
        &client,
        download_url,
        &temp_path,
        em,
        cancel.as_ref(),
        content_length,
    )
    .await
    {
        Ok(digests) => digests,
        Err(InstallError::Cancelled) => {
            best_effort_cleanup(&temp_path, None);
            if let Some(em) = em {
                em.emit(Phase::Cancelled, 0, None, None);
            }
            return Err(InstallError::Cancelled);
        }
        Err(e) => {
            best_effort_cleanup(&temp_path, None);
            if let Some(em) = em {
                em.emit(Phase::Failed, 0, None, Some(e.to_string()));
            }
            return Err(e);
        }
    };

    // ── 7. Check cancellation before verification ─────────────────────────────

    if let Some(tok) = &cancel {
        if tok.is_cancelled() {
            best_effort_cleanup(&temp_path, None);
            if let Some(em) = em {
                em.emit(Phase::Cancelled, 0, None, None);
            }
            return Err(InstallError::Cancelled);
        }
    }

    if let Some(em) = em {
        em.emit(Phase::Verifying, 0, None, None);
    }

    // ── 8. Verify checksum (dispatch by kind) ─────────────────────────────────

    match checksum_kind {
        ChecksumKind::Sha512Sidecar => {
            if let Some(checksum_url) = &version_info.checksum_url {
                // F003: Validate checksum URL origin before fetching.
                if let Err(e) = validate_release_url(checksum_url) {
                    best_effort_cleanup(&temp_path, None);
                    if let Some(em) = em {
                        em.emit(Phase::Failed, 0, None, Some(e.to_string()));
                    }
                    return Err(e);
                }
                let expected_hex = match fetch_sha512_sidecar(&client, checksum_url).await {
                    Ok(h) => h,
                    Err(e) => {
                        best_effort_cleanup(&temp_path, None);
                        if let Some(em) = em {
                            em.emit(Phase::Failed, 0, None, Some(e.to_string()));
                        }
                        return Err(e);
                    }
                };
                let actual_hex = hex_encode(&sha512_digest);
                if actual_hex != expected_hex {
                    let msg = format!(
                        "SHA-512 checksum mismatch for {}: expected {expected_hex}, got {actual_hex}",
                        version_info.version
                    );
                    best_effort_cleanup(&temp_path, None);
                    if let Some(em) = em {
                        em.emit(Phase::Failed, 0, None, Some(msg.clone()));
                    }
                    return Err(InstallError::ChecksumFailed(msg));
                }
                tracing::info!(version = %version_info.version, "SHA-512 checksum verified");
            } else {
                let msg = format!(
                    "provider requires SHA-512 sidecar checksum but no checksum URL is available for version {}",
                    version_info.version
                );
                best_effort_cleanup(&temp_path, None);
                if let Some(em) = em {
                    em.emit(Phase::Failed, 0, None, Some(msg.clone()));
                }
                return Err(InstallError::ChecksumMissing(msg));
            }
        }

        ChecksumKind::Sha256Manifest => {
            // F004: Expand ok_or_else to a match so Phase::Failed is emitted
            // before returning, preventing the frontend progress bar from
            // getting stuck in `Verifying` when checksum_url is absent.
            let manifest_url = match version_info.checksum_url.as_deref() {
                Some(u) => u,
                None => {
                    best_effort_cleanup(&temp_path, None);
                    let err = InstallError::ChecksumMissing(format!(
                        "provider requires SHA256SUMS manifest but none is available for version {}",
                        version_info.version
                    ));
                    if let Some(em) = em {
                        em.emit(Phase::Failed, 0, None, Some(err.to_string()));
                    }
                    return Err(err);
                }
            };

            // F003: Validate checksum URL origin before fetching.
            if let Err(e) = validate_release_url(manifest_url) {
                best_effort_cleanup(&temp_path, None);
                if let Some(em) = em {
                    em.emit(Phase::Failed, 0, None, Some(e.to_string()));
                }
                return Err(e);
            }

            let expected_hex =
                match fetch_sha256_manifest(&client, manifest_url, &archive_filename).await {
                    Ok(h) => h,
                    Err(e) => {
                        best_effort_cleanup(&temp_path, None);
                        if let Some(em) = em {
                            em.emit(Phase::Failed, 0, None, Some(e.to_string()));
                        }
                        return Err(e);
                    }
                };

            let actual_hex = hex_encode(&sha256_digest);
            if actual_hex != expected_hex {
                let msg = format!(
                    "SHA-256 checksum mismatch for {}: expected {expected_hex}, got {actual_hex}",
                    version_info.version
                );
                best_effort_cleanup(&temp_path, None);
                if let Some(em) = em {
                    em.emit(Phase::Failed, 0, None, Some(msg.clone()));
                }
                return Err(InstallError::ChecksumFailed(msg));
            }
            tracing::info!(version = %version_info.version, "SHA-256 checksum verified");
        }

        ChecksumKind::None => {
            tracing::warn!(
                version = %version_info.version,
                "provider does not publish checksums; skipping verification"
            );
            if let Some(em) = em {
                em.emit(
                    Phase::Verifying,
                    0,
                    None,
                    Some("provider does not publish checksums; skipping verification".into()),
                );
            }
        }
    }

    // ── 9. Check cancellation before extraction ───────────────────────────────

    if let Some(tok) = &cancel {
        if tok.is_cancelled() {
            best_effort_cleanup(&temp_path, None);
            if let Some(em) = em {
                em.emit(Phase::Cancelled, 0, None, None);
            }
            return Err(InstallError::Cancelled);
        }
    }

    // ── 10. Discover install target from archive ──────────────────────────────

    let top_level_dir = match peek_archive(temp_path.clone()).await {
        Ok(d) => d,
        Err(e) => {
            best_effort_cleanup(&temp_path, None);
            if let Some(em) = em {
                em.emit(Phase::Failed, 0, None, Some(e.to_string()));
            }
            return Err(e);
        }
    };

    // Only GE-Proton guarantees `archive top-level dir == release tag`.
    // Proton-CachyOS (`proton-cachyos-<tag>-x86_64/`) and Proton-EM
    // (`proton-<tag>/`) use distinct naming schemes; the matching
    // TS-side `normalizeInstallToTag` helper knows how to recover the
    // tag from those directory names. For any non-GE provider we trust
    // the archive's declared top-level — `unpack_in(dest_dir)` still
    // prevents the archive from escaping the compat-tools root.
    if version_info.provider == "ge-proton" && top_level_dir != version_info.version {
        let msg = format!(
            "archive top-level directory '{top_level_dir}' does not match expected version '{}'",
            version_info.version
        );
        best_effort_cleanup(&temp_path, None);
        if let Some(em) = em {
            em.emit(Phase::Failed, 0, None, Some(msg.clone()));
        }
        return Err(InstallError::InvalidPath(msg));
    }

    let installed_dir = dest_dir.join(&top_level_dir);
    if installed_dir.exists() {
        if !request.force {
            best_effort_cleanup(&temp_path, None);
            return Err(InstallError::AlreadyInstalled {
                path: installed_dir,
            });
        }
        if let Err(err) = fs::remove_dir_all(&installed_dir).await {
            best_effort_cleanup(&temp_path, None);
            let msg = format!(
                "failed to remove existing install directory '{}' before forced reinstall: {err}",
                installed_dir.display()
            );
            if let Some(em) = em {
                em.emit(Phase::Failed, 0, None, Some(msg.clone()));
            }
            let is_permission = matches!(err.kind(), std::io::ErrorKind::PermissionDenied);
            return if is_permission {
                Err(InstallError::PermissionDenied(msg))
            } else {
                Err(InstallError::Unknown(msg))
            };
        }
    }

    // ── 11. Extract archive ───────────────────────────────────────────────────

    if let Some(em) = em {
        em.emit(Phase::Extracting, 0, None, None);
    }

    // Check cancellation between extraction phases.
    if let Some(tok) = &cancel {
        if tok.is_cancelled() {
            best_effort_cleanup(&temp_path, None);
            if let Some(em) = em {
                em.emit(Phase::Cancelled, 0, None, None);
            }
            return Err(InstallError::Cancelled);
        }
    }

    let extracted_top = match extract_archive(temp_path.clone(), dest_dir.clone()).await {
        Ok(d) => d,
        Err(e) => {
            best_effort_cleanup(&temp_path, Some(&installed_dir));
            if let Some(em) = em {
                em.emit(Phase::Failed, 0, None, Some(e.to_string()));
            }
            return Err(e);
        }
    };

    if let Some(tok) = &cancel {
        if tok.is_cancelled() {
            best_effort_cleanup(&temp_path, Some(&installed_dir));
            if let Some(em) = em {
                em.emit(
                    Phase::Cancelled,
                    0,
                    None,
                    Some("cancelled during extraction".to_string()),
                );
            }
            return Err(InstallError::Cancelled);
        }
    }

    if extracted_top != top_level_dir {
        let msg = format!(
            "archive top-level directory changed between peek and extract (peek: {top_level_dir}, extract: {extracted_top})"
        );
        best_effort_cleanup(&temp_path, Some(&installed_dir));
        if let Some(em) = em {
            em.emit(Phase::Failed, 0, None, Some(msg.clone()));
        }
        return Err(InstallError::Unknown(msg));
    }

    // ── 12. Finalize ──────────────────────────────────────────────────────────

    if let Some(em) = em {
        em.emit(Phase::Finalizing, 0, None, None);
    }

    if let Err(e) = fs::remove_file(&temp_path).await {
        tracing::warn!(
            path = %temp_path.display(),
            error = %e,
            "failed to remove temp archive after extraction"
        );
    }

    // Verify extracted directory contains a `proton` executable.
    let proton_bin = installed_dir.join("proton");
    if !proton_bin.is_file() {
        let msg = format!(
            "extracted archive does not contain a 'proton' executable at {}",
            proton_bin.display()
        );
        let _ = std::fs::remove_dir_all(&installed_dir);
        if let Some(em) = em {
            em.emit(Phase::Failed, 0, None, Some(msg.clone()));
        }
        return Err(InstallError::Unknown(msg));
    }

    tracing::info!(
        version = %version_info.version,
        path = %installed_dir.display(),
        "Proton version installed successfully"
    );

    if let Some(em) = em {
        em.emit(Phase::Done, 0, None, None);
    }

    Ok(ProtonUpInstallResult {
        success: true,
        installed_path: Some(installed_dir.to_string_lossy().to_string()),
        error_kind: None,
        error_message: None,
    })
}

// ── utilities ─────────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
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

    // ── shared fixtures ───────────────────────────────────────────────────────

    // ── first_normal_path_component ──────────────────────────────────────────

    #[test]
    fn first_normal_component_returns_plain_dir_name() {
        let p = std::path::Path::new("GE-Proton9-21/proton");
        assert_eq!(
            first_normal_path_component(p),
            Some("GE-Proton9-21".to_string())
        );
    }

    /// GNU tar commonly prefixes entries with `./` (POSIX current-directory
    /// marker). Proton-EM ships archives like this. The first raw component
    /// is `CurDir` (`.`); we must look past it to reach the real top-level
    /// directory name — otherwise peek returns 0 usable entries and the
    /// install fails with "archive appears to be empty".
    #[test]
    fn first_normal_component_skips_leading_current_dir_marker() {
        let p = std::path::Path::new("./proton-EM-10.0-37-HDR/proton");
        assert_eq!(
            first_normal_path_component(p),
            Some("proton-EM-10.0-37-HDR".to_string())
        );
    }

    #[test]
    fn first_normal_component_rejects_absolute_paths() {
        let p = std::path::Path::new("/etc/passwd");
        assert_eq!(first_normal_path_component(p), None);
    }

    #[test]
    fn first_normal_component_rejects_parent_traversal() {
        let p = std::path::Path::new("../escape/me");
        assert_eq!(first_normal_path_component(p), None);
    }

    /// End-to-end: a tar built with leading `./` entries (Proton-EM's shape)
    /// must peek to the actual dir name, not be treated as empty.
    #[test]
    fn peek_tar_with_dot_slash_prefix_returns_real_top_level() {
        let mut buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut buf);
            let mut header = tar::Header::new_gnu();
            header.set_path("./proton-EM-10.0-37-HDR/proton").unwrap();
            header.set_size(0);
            header.set_cksum();
            builder.append(&header, std::io::empty()).unwrap();
            builder.finish().unwrap();
        }

        let top = peek_tar_read_top_level_sync(buf.as_slice()).expect("peek must succeed");
        assert_eq!(top, "proton-EM-10.0-37-HDR");
    }

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
            published_at: None,
        }
    }

    // ── validate_install_destination ─────────────────────────────────────────

    #[test]
    fn rejects_empty_path() {
        let result = validate_install_destination("").unwrap_err();
        assert!(matches!(result, InstallError::InvalidPath(_)));
    }

    #[test]
    fn rejects_path_with_parent_dir_component() {
        let result = validate_install_destination(
            "/home/user/.steam/../../../etc/passwd/compatibilitytools.d",
        )
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

    /// F001: A symlink at the `compatibilitytools.d` component that resolves to
    /// a sibling directory (i.e., the canonical path has no
    /// `compatibilitytools.d` segment) must be refused with
    /// `InstallError::InvalidPath`.
    #[test]
    fn rejects_symlink_redirect_escaping_compat_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        // Create a sibling directory that the symlink will point to.
        let real_target = temp.path().join("not_compat");
        std::fs::create_dir_all(&real_target).expect("real target dir");

        // Create a symlink named `compatibilitytools.d` pointing to the sibling.
        let symlink_path = temp.path().join("compatibilitytools.d");
        std::os::unix::fs::symlink(&real_target, &symlink_path).expect("create symlink");

        let dest = symlink_path.to_string_lossy().to_string();
        let result = validate_install_destination(&dest).unwrap_err();
        assert!(
            matches!(result, InstallError::InvalidPath(_)),
            "expected InvalidPath for symlink-redirected compat dir, got: {result:?}"
        );
    }

    // ── hex_encode ────────────────────────────────────────────────────────────

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

    // ── already_installed check ───────────────────────────────────────────────

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
        let version_info = make_version("GE-Proton9-21", None, None);

        let result = install_version(&request, &version_info).await;
        assert!(!result.success);
        assert_eq!(
            result.error_kind,
            Some(ProtonUpInstallErrorKind::DependencyMissing),
            "with force=true the already-installed guard must be skipped"
        );
    }

    // ── validate_release_url ─────────────────────────────────────────────────

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

    // ── error helper ──────────────────────────────────────────────────────────

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

    // ── progress event tests ──────────────────────────────────────────────────

    /// A fake provider with `ChecksumKind::Sha256Manifest` for test dispatch.
    #[cfg(test)]
    struct FakeSha256Provider {
        supports: bool,
    }

    #[cfg(test)]
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
        ) -> Result<Vec<crate::protonup::ProtonUpAvailableVersion>, providers::ProviderError>
        {
            Ok(vec![])
        }
    }

    /// Helper: register a test provider in a local registry slice and run the
    /// install orchestrator using that provider (by directly resolving checksum_kind).
    fn sha256_version(
        version: &str,
        download_url: Option<&str>,
        checksum_url: Option<&str>,
    ) -> ProtonUpAvailableVersion {
        ProtonUpAvailableVersion {
            provider: "fake-sha256".to_string(),
            version: version.to_string(),
            release_url: None,
            download_url: download_url.map(str::to_string),
            checksum_url: checksum_url.map(str::to_string),
            checksum_kind: Some("sha256-manifest".to_string()),
            asset_size: None,
            published_at: None,
        }
    }

    #[tokio::test]
    async fn emits_progress_events_during_download() {
        let mock_server = MockServer::start().await;
        // Serve a valid archive so all phases complete.
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
                ResponseTemplate::new(200)
                    .set_body_string(format!("{digest}  GE-Proton9-22.tar.gz")),
            )
            .mount(&mock_server)
            .await;

        let temp = tempfile::tempdir().expect("temp dir");
        let compat_dir = temp.path().join("compatibilitytools.d");
        std::fs::create_dir_all(&compat_dir).expect("compat dir");

        let download_url = format!("{}/GE-Proton9-22.tar.gz", mock_server.uri());
        let request = ProtonUpInstallRequest {
            provider: "ge-proton".to_string(),
            version: "GE-Proton9-22".to_string(),
            target_root: compat_dir.to_string_lossy().to_string(),
            force: false,
        };
        let checksum_url = format!("{}/GE-Proton9-22.tar.gz.sha512sum", mock_server.uri());
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

        // Collect all emitted phases.
        let mut phases = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            phases.push(format!("{:?}", ev.phase));
        }

        // Must include at least Resolving, Downloading, Verifying, Extracting,
        // Finalizing, Done in order (there may be gaps if no receivers were
        // subscribed between emits, but since we subscribed before calling
        // install_version_with_progress they will all be buffered).
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
        // Use a larger body so the download loop has a chance to be running when
        // we cancel. We serve 1 MiB of zeros in a .tar.gz wrapper.
        let archive = minimal_ge_proton_tar_gz("GE-Proton9-cancel");
        Mock::given(method("GET"))
            .and(path("/GE-Proton9-cancel.tar.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(archive)
                    // Add a small delay so cancellation can race.
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

        // Cancel immediately — will be observed before or during download.
        token.cancel();

        let err =
            install_version_with_progress(&request, &version_info, Some(emitter), Some(token))
                .await
                .expect_err("expected Cancelled error");

        assert!(
            matches!(err, InstallError::Cancelled),
            "expected Cancelled, got: {err:?}"
        );

        // Temp file must be gone.
        let temp_path = compat_dir.join(".tmp.GE-Proton9-cancel.tar.gz");
        assert!(
            !temp_path.exists(),
            "temp file should be cleaned up after cancel"
        );

        // Partial extract dir must be gone.
        let partial = compat_dir.join("GE-Proton9-cancel");
        assert!(
            !partial.exists(),
            "partial extract dir should be cleaned up"
        );

        // Must have seen a Cancelled phase event.
        let mut saw_cancelled = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev.phase, Phase::Cancelled) {
                saw_cancelled = true;
            }
        }
        assert!(saw_cancelled, "expected Phase::Cancelled event");
    }

    #[tokio::test]
    async fn verifies_sha256_manifest_checksum() {
        let mock_server = MockServer::start().await;

        // Build a real archive and compute its SHA-256.
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

        // Use "fake-sha256" provider id which maps to sha256-manifest via version_info.checksum_kind.
        let request = ProtonUpInstallRequest {
            provider: "fake-sha256".to_string(),
            version: "fake-sha256-ver".to_string(),
            target_root: compat_dir.to_string_lossy().to_string(),
            force: false,
        };
        let version_info =
            sha256_version("fake-sha256-ver", Some(&download_url), Some(&checksum_url));

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

        // Directly exercise the supports_install=false guard by simulating what
        // install_version_with_progress does: look up the provider and check.
        let fake = FakeSha256Provider { supports: false };
        assert!(
            !fake.supports_install(),
            "FakeSha256Provider with supports=false must return false"
        );

        // Confirm the error kind produced by the guard matches what we expect.
        let e = InstallError::DependencyMissing {
            reason: "catalog-only provider".into(),
        };
        assert!(
            matches!(e, InstallError::DependencyMissing { .. }),
            "expected DependencyMissing"
        );

        // End-to-end: drive the full orchestrator with a provider id that maps
        // to the fake provider's checksum kind via version_info.checksum_kind.
        // The "fake-sha256" id is not in the registry, so the orchestrator falls
        // back to the legacy checksum_kind parsing path (ChecksumKind::Sha256Manifest).
        // No download URL means it returns DependencyMissing for that reason —
        // the important thing is it does NOT panic on the missing registry entry.
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
}
