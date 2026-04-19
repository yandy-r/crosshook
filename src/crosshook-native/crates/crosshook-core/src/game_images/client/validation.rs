use std::path::{Path, PathBuf};

use infer::Infer;

use crate::game_images::models::GameImageError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub(super) const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MB
const ALLOWED_IMAGE_MIMES: &[&str] = &["image/jpeg", "image/png", "image/webp"];

// ---------------------------------------------------------------------------
// Security helpers (verbatim from research-security.md)
// ---------------------------------------------------------------------------

/// Validate downloaded image bytes by magic-byte detection.
///
/// Rejects any content larger than 5 MB (I4) and any MIME type outside the
/// explicit allow-list of `image/jpeg`, `image/png`, `image/webp` (I1 / I3).
/// SVG has no magic bytes and maps to `application/octet-stream` — it is
/// therefore unconditionally rejected.
pub fn validate_image_bytes(bytes: &[u8]) -> Result<&'static str, GameImageError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(GameImageError::TooLarge);
    }
    let infer = Infer::new();
    let mime = infer
        .get(bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");
    if !ALLOWED_IMAGE_MIMES.contains(&mime) {
        return Err(GameImageError::ForbiddenFormat(mime.to_string()));
    }
    Ok(mime)
}

pub(super) fn mime_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "img",
    }
}

/// Construct a safe, canonicalized path inside `base_dir`.
///
/// Validates `app_id` as a pure decimal integer and `filename` as a plain
/// basename with no path separators before resolving the final path (I2).
pub(super) fn safe_image_cache_path(
    base_dir: &Path,
    app_id: &str,
    filename: &str,
) -> Result<PathBuf, GameImageError> {
    // app_id must be a pure decimal integer — no slashes, no dots
    if !app_id.chars().all(|c| c.is_ascii_digit()) || app_id.is_empty() {
        return Err(GameImageError::InvalidAppId);
    }
    // filename must be a safe basename — no path separators
    let fname = Path::new(filename);
    if fname.components().count() != 1 {
        return Err(GameImageError::InvalidFilename);
    }
    // Resolve canonical base, then assert prefix
    let canonical_base = std::fs::canonicalize(base_dir)?;
    let joined = canonical_base.join(app_id).join(filename);
    // joined is not yet canonical (subdirs may not exist); strip to parent
    let parent = joined.parent().ok_or(GameImageError::InvalidPath)?;
    std::fs::create_dir_all(parent)?;
    let canonical_parent = std::fs::canonicalize(parent)?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(GameImageError::PathEscaped);
    }
    Ok(joined)
}
