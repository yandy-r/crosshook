use std::path::Path;

use tokio::fs;
use tokio_util::sync::CancellationToken;

use crate::protonup::install_root::{pick_default_install_root, resolve_install_root_candidates};
use crate::protonup::progress::{Phase, ProgressEmitter};
use crate::protonup::providers::{self, ChecksumKind};
use crate::protonup::{ProtonUpAvailableVersion, ProtonUpInstallRequest, ProtonUpInstallResult};

use super::archive::{best_effort_cleanup, extract_archive, peek_archive};
use super::download::{download_to_file, fetch_sha256_manifest, fetch_sha512_sidecar, hex_encode};
use super::errors::InstallError;
use super::validation::{
    validate_archive_filename, validate_install_destination, validate_release_url,
};

/// Backward-compatible install entry point. Delegates to
/// [`install_version_with_progress`] with no emitter or cancel token.
pub async fn install_version(
    request: &ProtonUpInstallRequest,
    version_info: &ProtonUpAvailableVersion,
) -> ProtonUpInstallResult {
    match install_version_with_progress(request, version_info, None, None).await {
        Ok(result) => result,
        Err(error) => error.to_result(),
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
    emit(em, Phase::Resolving, None);

    let checksum_kind = resolve_checksum_kind(request, version_info)?;
    let effective_root = resolve_target_root(request)?;
    let dest_dir = validate_install_destination(&effective_root)?;
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

    validate_release_url(download_url)?;

    let archive_filename = download_url
        .rsplit('/')
        .next()
        .unwrap_or("proton-archive.tar.gz")
        .to_string();
    validate_archive_filename(&archive_filename)?;
    let temp_path = dest_dir.join(format!(".tmp.{archive_filename}"));
    let client = reqwest::Client::new();

    if cancellation_requested(cancel.as_ref()) {
        return cancel_with_cleanup(&temp_path, None, em, None);
    }

    let (sha512_digest, sha256_digest) = match download_to_file(
        &client,
        download_url,
        &temp_path,
        em,
        cancel.as_ref(),
        version_info.asset_size,
    )
    .await
    {
        Ok(digests) => digests,
        Err(InstallError::Cancelled) => return cancel_with_cleanup(&temp_path, None, em, None),
        Err(error) => return fail_with_cleanup(&temp_path, None, em, error),
    };

    if cancellation_requested(cancel.as_ref()) {
        return cancel_with_cleanup(&temp_path, None, em, None);
    }

    emit(em, Phase::Verifying, None);
    if let Err(error) = verify_checksum(
        checksum_kind,
        &client,
        version_info,
        &archive_filename,
        &temp_path,
        em,
        &sha512_digest,
        &sha256_digest,
    )
    .await
    {
        return fail_with_cleanup_message(
            &temp_path,
            None,
            em,
            error.error,
            error.progress_message,
        );
    }

    if cancellation_requested(cancel.as_ref()) {
        return cancel_with_cleanup(&temp_path, None, em, None);
    }

    let top_level_dir = match peek_archive(temp_path.clone()).await {
        Ok(dir) => dir,
        Err(error) => return fail_with_cleanup(&temp_path, None, em, error),
    };
    if let Err((error, progress_message)) = validate_archive_top_level(version_info, &top_level_dir)
    {
        return fail_with_cleanup_message(&temp_path, None, em, error, Some(progress_message));
    }

    let installed_dir = dest_dir.join(&top_level_dir);
    if installed_dir.exists() {
        if !request.force {
            best_effort_cleanup(&temp_path, None);
            return Err(InstallError::AlreadyInstalled {
                path: installed_dir,
            });
        }
        if let Err((error, progress_message)) = remove_existing_install_dir(&installed_dir).await {
            return fail_with_cleanup_message(&temp_path, None, em, error, Some(progress_message));
        }
    }

    emit(em, Phase::Extracting, None);
    if cancellation_requested(cancel.as_ref()) {
        return cancel_with_cleanup(&temp_path, None, em, None);
    }

    let extracted_top = match extract_archive(temp_path.clone(), dest_dir.clone()).await {
        Ok(dir) => dir,
        Err(error) => return fail_with_cleanup(&temp_path, Some(&installed_dir), em, error),
    };

    if cancellation_requested(cancel.as_ref()) {
        return cancel_with_cleanup(
            &temp_path,
            Some(&installed_dir),
            em,
            Some("cancelled during extraction".to_string()),
        );
    }

    if extracted_top != top_level_dir {
        let message = format!(
            "archive top-level directory changed between peek and extract (peek: {top_level_dir}, extract: {extracted_top})"
        );
        return fail_with_cleanup_message(
            &temp_path,
            Some(&installed_dir),
            em,
            InstallError::Unknown(message.clone()),
            Some(message),
        );
    }

    emit(em, Phase::Finalizing, None);

    if let Err(error) = fs::remove_file(&temp_path).await {
        tracing::warn!(
            path = %temp_path.display(),
            error = %error,
            "failed to remove temp archive after extraction"
        );
    }

    if let Err(error) = validate_proton_binary(&installed_dir) {
        let _ = std::fs::remove_dir_all(&installed_dir);
        let progress_message = match &error {
            InstallError::Unknown(message) => Some(message.clone()),
            _ => None,
        };
        return fail_with_cleanup_message(
            &temp_path,
            Some(&installed_dir),
            em,
            error,
            progress_message,
        );
    }

    tracing::info!(
        version = %version_info.version,
        path = %installed_dir.display(),
        "Proton version installed successfully"
    );
    emit(em, Phase::Done, None);

    Ok(ProtonUpInstallResult {
        success: true,
        installed_path: Some(installed_dir.to_string_lossy().to_string()),
        error_kind: None,
        error_message: None,
    })
}

fn emit(emitter: Option<&ProgressEmitter>, phase: Phase, message: Option<String>) {
    if let Some(emitter) = emitter {
        emitter.emit(phase, 0, None, message);
    }
}

fn cancellation_requested(cancel: Option<&CancellationToken>) -> bool {
    cancel.is_some_and(CancellationToken::is_cancelled)
}

fn resolve_checksum_kind(
    request: &ProtonUpInstallRequest,
    version_info: &ProtonUpAvailableVersion,
) -> Result<ChecksumKind, InstallError> {
    let registry = providers::registry();
    let provider = registry
        .iter()
        .find(|provider| provider.id() == request.provider.as_str());

    if let Some(provider) = provider {
        if !provider.supports_install() {
            return Err(InstallError::DependencyMissing {
                reason: "catalog-only provider".into(),
            });
        }
        return Ok(provider.checksum_kind());
    }

    Ok(match version_info.checksum_kind.as_deref() {
        Some("sha512") | Some("sha512-sidecar") => ChecksumKind::Sha512Sidecar,
        Some("sha256") | Some("sha256-manifest") => ChecksumKind::Sha256Manifest,
        _ => ChecksumKind::None,
    })
}

fn resolve_target_root(request: &ProtonUpInstallRequest) -> Result<String, InstallError> {
    if !request.target_root.trim().is_empty() {
        return Ok(request.target_root.clone());
    }

    let steam_path = None::<&Path>;
    let candidates = resolve_install_root_candidates(steam_path);
    match pick_default_install_root(&candidates) {
        Some(candidate) if candidate.writable => Ok(candidate.path.to_string_lossy().to_string()),
        _ => Err(InstallError::NoWritableInstallRoot),
    }
}

struct StepError {
    error: InstallError,
    progress_message: Option<String>,
}

impl From<InstallError> for StepError {
    fn from(error: InstallError) -> Self {
        Self {
            error,
            progress_message: None,
        }
    }
}

impl StepError {
    fn with_progress_message(error: InstallError, progress_message: String) -> Self {
        Self {
            error,
            progress_message: Some(progress_message),
        }
    }
}

async fn verify_checksum(
    checksum_kind: ChecksumKind,
    client: &reqwest::Client,
    version_info: &ProtonUpAvailableVersion,
    archive_filename: &str,
    temp_path: &Path,
    emitter: Option<&ProgressEmitter>,
    sha512_digest: &[u8],
    sha256_digest: &[u8],
) -> Result<(), StepError> {
    match checksum_kind {
        ChecksumKind::Sha512Sidecar => {
            let checksum_url =
                version_info
                    .checksum_url
                    .as_deref()
                    .ok_or_else(|| {
                        let message = format!(
                            "provider requires SHA-512 sidecar checksum but no checksum URL is available for version {}",
                            version_info.version
                        );
                        StepError::with_progress_message(
                            InstallError::ChecksumMissing(message.clone()),
                            message,
                        )
                    })?;

            validate_release_url(checksum_url).map_err(StepError::from)?;
            let expected_hex = fetch_sha512_sidecar(client, checksum_url)
                .await
                .map_err(StepError::from)?;
            let actual_hex = hex_encode(sha512_digest);
            if actual_hex != expected_hex {
                let message = format!(
                    "SHA-512 checksum mismatch for {}: expected {expected_hex}, got {actual_hex}",
                    version_info.version
                );
                return Err(StepError::with_progress_message(
                    InstallError::ChecksumFailed(message.clone()),
                    message,
                ));
            }

            tracing::info!(version = %version_info.version, "SHA-512 checksum verified");
            Ok(())
        }
        ChecksumKind::Sha256Manifest => {
            let manifest_url = version_info
                .checksum_url
                .as_deref()
                .ok_or_else(|| {
                    InstallError::ChecksumMissing(format!(
                    "provider requires SHA256SUMS manifest but none is available for version {}",
                    version_info.version
                ))
                })
                .map_err(StepError::from)?;

            validate_release_url(manifest_url).map_err(StepError::from)?;
            let expected_hex = fetch_sha256_manifest(client, manifest_url, archive_filename)
                .await
                .map_err(StepError::from)?;
            let actual_hex = hex_encode(sha256_digest);
            if actual_hex != expected_hex {
                let message = format!(
                    "SHA-256 checksum mismatch for {}: expected {expected_hex}, got {actual_hex}",
                    version_info.version
                );
                return Err(StepError::with_progress_message(
                    InstallError::ChecksumFailed(message.clone()),
                    message,
                ));
            }

            tracing::info!(version = %version_info.version, "SHA-256 checksum verified");
            Ok(())
        }
        ChecksumKind::None => {
            tracing::warn!(
                version = %version_info.version,
                "provider does not publish checksums; skipping verification"
            );
            let _ = temp_path;
            emit(
                emitter,
                Phase::Verifying,
                Some("provider does not publish checksums; skipping verification".into()),
            );
            Ok(())
        }
    }
}

fn validate_archive_top_level(
    version_info: &ProtonUpAvailableVersion,
    top_level_dir: &str,
) -> Result<(), (InstallError, String)> {
    if version_info.provider == "ge-proton" && top_level_dir != version_info.version {
        let message = format!(
            "archive top-level directory '{top_level_dir}' does not match expected version '{}'",
            version_info.version
        );
        return Err((InstallError::InvalidPath(message.clone()), message));
    }

    Ok(())
}

async fn remove_existing_install_dir(installed_dir: &Path) -> Result<(), (InstallError, String)> {
    if let Err(error) = fs::remove_dir_all(installed_dir).await {
        let message = format!(
            "failed to remove existing install directory '{}' before forced reinstall: {error}",
            installed_dir.display()
        );
        return if error.kind() == std::io::ErrorKind::PermissionDenied {
            Err((InstallError::PermissionDenied(message.clone()), message))
        } else {
            Err((InstallError::Unknown(message.clone()), message))
        };
    }

    Ok(())
}

fn validate_proton_binary(installed_dir: &Path) -> Result<(), InstallError> {
    let proton_bin = installed_dir.join("proton");
    if !proton_bin.is_file() {
        return Err(InstallError::Unknown(format!(
            "extracted archive does not contain a 'proton' executable at {}",
            proton_bin.display()
        )));
    }

    Ok(())
}

fn fail_with_cleanup<T>(
    temp_path: &Path,
    partial_dir: Option<&Path>,
    emitter: Option<&ProgressEmitter>,
    error: InstallError,
) -> Result<T, InstallError> {
    fail_with_cleanup_message(temp_path, partial_dir, emitter, error, None)
}

fn fail_with_cleanup_message<T>(
    temp_path: &Path,
    partial_dir: Option<&Path>,
    emitter: Option<&ProgressEmitter>,
    error: InstallError,
    progress_message: Option<String>,
) -> Result<T, InstallError> {
    best_effort_cleanup(temp_path, partial_dir);
    emit(
        emitter,
        Phase::Failed,
        Some(progress_message.unwrap_or_else(|| error.to_string())),
    );
    Err(error)
}

fn cancel_with_cleanup<T>(
    temp_path: &Path,
    partial_dir: Option<&Path>,
    emitter: Option<&ProgressEmitter>,
    message: Option<String>,
) -> Result<T, InstallError> {
    best_effort_cleanup(temp_path, partial_dir);
    emit(emitter, Phase::Cancelled, message);
    Err(InstallError::Cancelled)
}
