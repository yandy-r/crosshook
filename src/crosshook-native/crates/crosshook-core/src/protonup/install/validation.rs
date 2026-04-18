use std::path::{Component, PathBuf};

use crate::settings::expand_path_with_tilde;

use super::errors::InstallError;

/// Validate that `target_root` is a legitimate Steam compatibility tools
/// directory and return the canonicalized path.
pub(super) fn validate_install_destination(target_root: &str) -> Result<PathBuf, InstallError> {
    let raw = target_root.trim();
    if raw.is_empty() {
        return Err(InstallError::InvalidPath(
            "install destination path is empty".into(),
        ));
    }

    let path = if raw.starts_with('~') {
        expand_path_with_tilde(raw)
            .map_err(|error| InstallError::InvalidPath(format!("failed to expand path: {error}")))?
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

    let has_compat_segment = path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("compatibilitytools.d")
    });
    if !has_compat_segment {
        return Err(InstallError::InvalidPath(
            "install destination must be under a 'compatibilitytools.d' directory".into(),
        ));
    }

    if path.exists() && !path.is_dir() {
        return Err(InstallError::InvalidPath(format!(
            "install destination '{}' exists and is not a directory",
            path.display()
        )));
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

    let canonical_has_compat_segment = canonical.components().any(|component| {
        component
            .as_os_str()
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

pub(super) fn validate_archive_filename(archive_filename: &str) -> Result<(), InstallError> {
    if archive_filename.is_empty()
        || archive_filename.contains('/')
        || archive_filename.contains('\\')
        || archive_filename.contains('\0')
        || archive_filename.contains("..")
    {
        return Err(InstallError::InvalidPath(
            "download URL resolved to an unsafe archive filename".into(),
        ));
    }

    Ok(())
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
pub(super) fn validate_release_url(url: &str) -> Result<(), InstallError> {
    const ALLOWED_HOSTS: &[&str] = &[
        "github.com",
        "api.github.com",
        "objects.githubusercontent.com",
        "github-releases.githubusercontent.com",
    ];

    let parsed = reqwest::Url::parse(url).map_err(|error| {
        InstallError::UntrustedUrl(format!("failed to parse URL '{url}': {error}"))
    })?;

    let host = parsed.host_str().unwrap_or("");

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
