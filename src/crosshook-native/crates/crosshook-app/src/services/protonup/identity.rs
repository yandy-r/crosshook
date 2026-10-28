//! Pure catalog-identity check for Proton installs.

use crosshook_core::protonup::{ProtonUpAvailableVersion, ProtonUpInstallRequest};

/// Why an install request does not match the server catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// No catalog entry has the requested provider and version.
    Missing,
    /// The supplied entry names a different provider or version than the catalog entry.
    Mismatch,
    /// The supplied download or checksum metadata differs from the catalog entry.
    MetadataMismatch,
}

/// Resolves `request` to its authoritative entry in `catalog`.
///
/// The entry is the first one whose provider and version equal the request's.
/// When the caller also `supplied` an entry, its provider and version must
/// equal the catalog entry's, and its `download_url`, `checksum_url` and
/// `checksum_kind` must be exactly equal (no URL normalization).
/// `release_url`, `asset_size` and `published_at` are never compared. Callers
/// must install the returned catalog entry, never `supplied`.
pub fn verify_install_identity<'a>(
    request: &ProtonUpInstallRequest,
    supplied: Option<&ProtonUpAvailableVersion>,
    catalog: &'a [ProtonUpAvailableVersion],
) -> Result<&'a ProtonUpAvailableVersion, IdentityError> {
    let entry = catalog
        .iter()
        .find(|v| v.version == request.version && v.provider == request.provider)
        .ok_or(IdentityError::Missing)?;

    if let Some(supplied) = supplied {
        if supplied.provider != entry.provider || supplied.version != entry.version {
            return Err(IdentityError::Mismatch);
        }
        if supplied.download_url != entry.download_url
            || supplied.checksum_url != entry.checksum_url
            || supplied.checksum_kind != entry.checksum_kind
        {
            return Err(IdentityError::MetadataMismatch);
        }
    }

    Ok(entry)
}
