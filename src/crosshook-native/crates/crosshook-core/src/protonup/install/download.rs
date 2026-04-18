use std::path::Path;

use sha2::{Digest, Sha256, Sha512};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::protonup::progress::{Phase, ProgressEmitter};

use super::errors::{map_io_err, network_err, InstallError};

/// Stream the URL into `dest_path`, returning the SHA-512 and SHA-256 digests.
/// Emits `Phase::Downloading` progress every `EMIT_INTERVAL_BYTES`.
const EMIT_INTERVAL_BYTES: u64 = 256 * 1024;

/// Hard ceiling for checksum sidecar / manifest response bodies (1 MiB).
const MAX_CHECKSUM_BYTES: u64 = 1024 * 1024;

pub(super) async fn download_to_file(
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
        .map_err(|error| network_err(format!("request failed for {url}: {error}")))?;

    if !response.status().is_success() {
        return Err(network_err(format!(
            "server returned {} for {url}",
            response.status()
        )));
    }

    let bytes_total = content_length.or_else(|| response.content_length());
    let mut file = fs::File::create(dest_path).await.map_err(|error| {
        map_io_err(
            error,
            &format!("failed to create temp file {}", dest_path.display()),
        )
    })?;

    let mut sha512 = Sha512::new();
    let mut sha256 = Sha256::new();
    let mut bytes_done = 0_u64;
    let mut since_last_emit = 0_u64;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;

    loop {
        let chunk = if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err(InstallError::Cancelled);
            }
            tokio::select! {
                biased;
                _ = token.cancelled() => return Err(InstallError::Cancelled),
                next = stream.next() => match next {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(error)) => {
                        return Err(network_err(format!("download interrupted: {error}")))
                    }
                    None => break,
                },
            }
        } else {
            match stream.next().await {
                Some(Ok(bytes)) => bytes,
                Some(Err(error)) => {
                    return Err(network_err(format!("download interrupted: {error}")))
                }
                None => break,
            }
        };

        sha512.update(&chunk);
        sha256.update(&chunk);
        file.write_all(&chunk).await.map_err(|error| {
            map_io_err(error, &format!("write failed to {}", dest_path.display()))
        })?;

        bytes_done += chunk.len() as u64;
        since_last_emit += chunk.len() as u64;

        if since_last_emit >= EMIT_INTERVAL_BYTES {
            since_last_emit = 0;
            if let Some(emitter) = emitter {
                emitter.emit(Phase::Downloading, bytes_done, bytes_total, None);
            }
        }
    }

    file.flush()
        .await
        .map_err(|error| map_io_err(error, &format!("flush failed for {}", dest_path.display())))?;
    drop(file);

    if bytes_done == 0 {
        return Err(network_err(format!(
            "download produced 0 bytes for {url} — server may have redirected to an empty response"
        )));
    }

    if let Some(expected) = bytes_total {
        if expected > 0 && bytes_done != expected {
            return Err(network_err(format!(
                "download truncated for {url}: got {bytes_done} bytes, expected {expected}"
            )));
        }
    }

    if let Some(emitter) = emitter {
        emitter.emit(Phase::Downloading, bytes_done, bytes_total, None);
    }

    Ok((sha512.finalize().to_vec(), sha256.finalize().to_vec()))
}

pub(super) async fn fetch_sha512_sidecar(
    client: &reqwest::Client,
    checksum_url: &str,
) -> Result<String, InstallError> {
    let response = client.get(checksum_url).send().await.map_err(|error| {
        network_err(format!(
            "checksum request failed for {checksum_url}: {error}"
        ))
    })?;

    if let Some(length) = response.content_length() {
        if length > MAX_CHECKSUM_BYTES {
            return Err(InstallError::ChecksumFailed(format!(
                "checksum response for {checksum_url} is too large ({length} bytes, limit {MAX_CHECKSUM_BYTES})"
            )));
        }
    }

    let body = response
        .text()
        .await
        .map_err(|error| network_err(format!("failed to read checksum body: {error}")))?;

    body.lines()
        .find_map(|line| {
            let (hash_part, _rest) = line.split_once("  ")?;
            let trimmed = hash_part.trim();
            if trimmed.len() == 128 && trimmed.chars().all(|char| char.is_ascii_hexdigit()) {
                Some(trimmed.to_lowercase())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            InstallError::ChecksumFailed(format!(
                "could not parse SHA-512 hash from checksum file at {checksum_url}"
            ))
        })
}

/// Fetch a `SHA256SUMS` manifest and return the hex hash for `asset_filename`.
///
/// Supports both `<hex>  <filename>` (two-space) and `<hex> *<filename>` formats.
pub(super) async fn fetch_sha256_manifest(
    client: &reqwest::Client,
    manifest_url: &str,
    asset_filename: &str,
) -> Result<String, InstallError> {
    let response = client.get(manifest_url).send().await.map_err(|error| {
        network_err(format!(
            "SHA256SUMS request failed for {manifest_url}: {error}"
        ))
    })?;

    if let Some(length) = response.content_length() {
        if length > MAX_CHECKSUM_BYTES {
            return Err(InstallError::ChecksumFailed(format!(
                "SHA256SUMS response for {manifest_url} is too large ({length} bytes, limit {MAX_CHECKSUM_BYTES})"
            )));
        }
    }

    let body = response
        .text()
        .await
        .map_err(|error| network_err(format!("failed to read SHA256SUMS body: {error}")))?;

    body.lines()
        .find_map(|line| {
            let (hash_part, rest) = line.split_once("  ").or_else(|| line.split_once(" *"))?;
            let trimmed = hash_part.trim();
            let filename = rest.trim();
            if filename == asset_filename
                && trimmed.len() == 64
                && trimmed.chars().all(|char| char.is_ascii_hexdigit())
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
        })
}

pub(super) fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, byte| {
            use std::fmt::Write;
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}
