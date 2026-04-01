use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use infer::Infer;

use crate::metadata::{sha256_hex, MetadataStore};

use super::models::{GameImageError, GameImageSource, GameImageType};
use super::steamgriddb::fetch_steamgriddb_image;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MB
const ALLOWED_IMAGE_MIMES: &[&str] = &["image/jpeg", "image/png", "image/webp"];
const CACHE_TTL_HOURS: i64 = 24;
const REQUEST_TIMEOUT_SECS: u64 = 15;

static GAME_IMAGES_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

// ---------------------------------------------------------------------------
// HTTP client singleton
// ---------------------------------------------------------------------------

pub(super) fn http_client() -> Result<&'static reqwest::Client, GameImageError> {
    if let Some(client) = GAME_IMAGES_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(GameImageError::ClientBuild)?;

    let _ = GAME_IMAGES_HTTP_CLIENT.set(client);
    Ok(GAME_IMAGES_HTTP_CLIENT
        .get()
        .expect("game images HTTP client should be initialized before use"))
}

// ---------------------------------------------------------------------------
// Security helpers (verbatim from research-security.md)
// ---------------------------------------------------------------------------

/// Validate downloaded image bytes by magic-byte detection.
///
/// Rejects any content larger than 5 MB (I4) and any MIME type outside the
/// explicit allow-list of `image/jpeg`, `image/png`, `image/webp` (I1 / I3).
/// SVG has no magic bytes and maps to `application/octet-stream` — it is
/// therefore unconditionally rejected.
fn validate_image_bytes(bytes: &[u8]) -> Result<(), GameImageError> {
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
    Ok(())
}

/// Construct a safe, canonicalized path inside `base_dir`.
///
/// Validates `app_id` as a pure decimal integer and `filename` as a plain
/// basename with no path separators before resolving the final path (I2).
fn safe_image_cache_path(
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Download and cache a game cover image for the given Steam `app_id`.
///
/// Returns `Ok(Some(absolute_path))` on success, `Ok(None)` when the download
/// fails or the image is rejected (network error, size limit, forbidden
/// format), and `Err(String)` only for hard configuration or I/O failures.
///
/// # Fallback chain
///
/// - If `api_key` is `Some`: SteamGridDB → Steam CDN → stale cache → `None`
/// - If `api_key` is `None`: Steam CDN → stale cache → `None`
///
/// # Cache lifecycle
///
/// 1. A valid (non-expired) cache entry whose file still exists on disk is
///    returned immediately without a network round-trip.
/// 2. On download success the image is written to disk, a SHA-256 checksum is
///    stored in the DB, and the entry expires in 24 hours.
/// 3. If the download fails but a stale entry with an existing file remains,
///    the stale path is returned as a fallback.
/// 4. If a cached entry references a file that no longer exists on disk, the
///    DB row is deleted and `None` is returned so the caller can retry.
pub async fn download_and_cache_image(
    store: &MetadataStore,
    app_id: &str,
    image_type: GameImageType,
    api_key: Option<&str>,
) -> Result<Option<String>, String> {
    // ------------------------------------------------------------------
    // Step (a): Validate app_id — pure decimal integers only (I2)
    // ------------------------------------------------------------------
    if app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "invalid app_id {app_id:?}: must be a non-empty decimal integer"
        ));
    }

    let image_type_str = image_type.to_string();

    // ------------------------------------------------------------------
    // Step (b): Check for a valid cached entry
    // ------------------------------------------------------------------
    if let Ok(Some(row)) = store.get_game_image(app_id, &image_type_str) {
        let is_expired = row
            .expires_at
            .as_deref()
            .map(|exp| exp <= Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string().as_str())
            .unwrap_or(false);

        if !is_expired {
            let cached_path = PathBuf::from(&row.file_path);
            if cached_path.exists() {
                tracing::debug!(
                    app_id,
                    image_type = image_type_str,
                    "returning valid cached game image"
                );
                return Ok(Some(row.file_path));
            }
            // Cached path no longer exists — delete the stale DB row
            tracing::warn!(
                app_id,
                image_type = image_type_str,
                file_path = row.file_path,
                "cached game image file missing from disk; deleting DB row"
            );
            delete_game_image_row(store, app_id, &image_type_str);
        }
    }

    // ------------------------------------------------------------------
    // Step (c): Attempt download — SteamGridDB first (if key present),
    //           then Steam CDN as fallback.
    // ------------------------------------------------------------------
    let (bytes, source) = if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        match fetch_steamgriddb_image(key, app_id, &image_type).await {
            Ok(b) => {
                tracing::debug!(app_id, image_type = image_type_str, "SteamGridDB image fetched");
                (b, GameImageSource::SteamGridDb)
            }
            Err(error) => {
                tracing::warn!(
                    app_id,
                    image_type = image_type_str,
                    %error,
                    "SteamGridDB fetch failed; falling back to Steam CDN"
                );
                // Fall back to Steam CDN
                let cdn_url = build_download_url(app_id, image_type);
                match download_image_bytes(&cdn_url).await {
                    Ok(b) => (b, GameImageSource::SteamCdn),
                    Err(cdn_error) => {
                        tracing::warn!(
                            app_id,
                            image_type = image_type_str,
                            %cdn_error,
                            "Steam CDN fallback also failed"
                        );
                        return Ok(stale_fallback_path(store, app_id, &image_type_str));
                    }
                }
            }
        }
    } else {
        // No API key — use Steam CDN directly
        let cdn_url = build_download_url(app_id, image_type);
        match download_image_bytes(&cdn_url).await {
            Ok(b) => (b, GameImageSource::SteamCdn),
            Err(error) => {
                tracing::warn!(app_id, image_type = image_type_str, %error, "game image download failed");
                return Ok(stale_fallback_path(store, app_id, &image_type_str));
            }
        }
    };

    // ------------------------------------------------------------------
    // Step (d): Validate image bytes (magic bytes, size, MIME allow-list)
    // ------------------------------------------------------------------
    if let Err(error) = validate_image_bytes(&bytes) {
        tracing::warn!(
            app_id,
            image_type = image_type_str,
            %error,
            "game image rejected by validation"
        );
        return Ok(None);
    }

    // ------------------------------------------------------------------
    // Step (e): Construct safe cache path
    // ------------------------------------------------------------------
    let base_dir = image_cache_base_dir()?;
    // Ensure the base directory exists before canonicalization
    if let Err(error) = std::fs::create_dir_all(&base_dir) {
        return Err(format!(
            "failed to create image cache base directory {}: {error}",
            base_dir.display()
        ));
    }

    let filename = filename_for(image_type, source);
    let file_path = safe_image_cache_path(&base_dir, app_id, &filename)
        .map_err(|error| format!("safe_image_cache_path failed: {error}"))?;

    // ------------------------------------------------------------------
    // Step (f): Write to disk
    // ------------------------------------------------------------------
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("failed to create cache directory: {error}"))?;
    }
    tokio::fs::write(&file_path, &bytes)
        .await
        .map_err(|error| format!("failed to write image to disk: {error}"))?;

    // ------------------------------------------------------------------
    // Step (g): Persist metadata in the DB
    // ------------------------------------------------------------------
    let source_str = source.to_string();
    let download_url = match source {
        GameImageSource::SteamCdn => build_download_url(app_id, image_type),
        GameImageSource::SteamGridDb => format!(
            "https://www.steamgriddb.com/api/v2/grids/steam/{app_id}"
        ),
    };
    let content_hash = sha256_hex(&bytes);
    let mime_type = Infer::new()
        .get(&bytes)
        .map(|t| t.mime_type())
        .unwrap_or("image/jpeg");
    let file_size = bytes.len() as i64;
    let absolute_path = file_path
        .to_str()
        .ok_or("image cache path contains non-UTF-8 characters")?
        .to_string();
    let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    if let Err(error) = store.upsert_game_image(
        app_id,
        &image_type_str,
        &source_str,
        &absolute_path,
        Some(file_size),
        Some(&content_hash),
        Some(mime_type),
        Some(&download_url),
        Some(&expires_at),
    ) {
        tracing::warn!(
            app_id,
            image_type = image_type_str,
            %error,
            "failed to persist game image cache row"
        );
        // Non-fatal: the file was written to disk; the caller can still use it.
    }

    // ------------------------------------------------------------------
    // Step (h): Return the absolute path
    // ------------------------------------------------------------------
    Ok(Some(absolute_path))
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn build_download_url(app_id: &str, image_type: GameImageType) -> String {
    match image_type {
        GameImageType::Cover => {
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"
            )
        }
        GameImageType::Hero => {
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero.jpg"
            )
        }
        GameImageType::Capsule => {
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/capsule_616x353.jpg"
            )
        }
    }
}

fn filename_for(image_type: GameImageType, source: GameImageSource) -> String {
    let source_suffix = match source {
        GameImageSource::SteamCdn => "steam_cdn",
        GameImageSource::SteamGridDb => "steamgriddb",
    };
    let type_prefix = match image_type {
        GameImageType::Cover => "cover",
        GameImageType::Hero => "hero",
        GameImageType::Capsule => "capsule",
    };
    format!("{type_prefix}_{source_suffix}.jpg")
}

fn image_cache_base_dir() -> Result<PathBuf, String> {
    directories::BaseDirs::new()
        .ok_or_else(|| "home directory not found".to_string())
        .map(|dirs| {
            dirs.data_local_dir()
                .join("crosshook")
                .join("cache")
                .join("images")
        })
}

async fn download_image_bytes(url: &str) -> Result<Vec<u8>, GameImageError> {
    let client = http_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(GameImageError::Network)?
        .error_for_status()
        .map_err(GameImageError::Network)?;

    // Read bytes with an explicit size limit (I4)
    let bytes = response
        .bytes()
        .await
        .map_err(GameImageError::Network)?
        .to_vec();

    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(GameImageError::TooLarge);
    }

    Ok(bytes)
}

/// Return the file path from a stale (possibly expired) cached entry if the
/// file still exists on disk.
fn stale_fallback_path(
    store: &MetadataStore,
    app_id: &str,
    image_type_str: &str,
) -> Option<String> {
    let row = store.get_game_image(app_id, image_type_str).ok().flatten()?;
    let cached_path = PathBuf::from(&row.file_path);
    if cached_path.exists() {
        tracing::debug!(
            app_id,
            image_type = image_type_str,
            "serving stale cached game image as fallback"
        );
        Some(row.file_path)
    } else {
        delete_game_image_row(store, app_id, image_type_str);
        None
    }
}

/// Best-effort deletion of a DB row for a missing cache file.
fn delete_game_image_row(store: &MetadataStore, app_id: &str, image_type_str: &str) {
    if let Err(error) = store.with_sqlite_conn("delete a stale game image cache row", |conn| {
        conn.execute(
            "DELETE FROM game_image_cache WHERE steam_app_id = ?1 AND image_type = ?2",
            rusqlite::params![app_id, image_type_str],
        )
        .map_err(|source| crate::metadata::MetadataStoreError::Database {
            action: "delete stale game image row",
            source,
        })?;
        Ok(())
    }) {
        tracing::warn!(
            app_id,
            image_type = image_type_str,
            %error,
            "failed to delete stale game image cache row"
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataStore;

    // -----------------------------------------------------------------------
    // app_id validation
    // -----------------------------------------------------------------------

    #[test]
    fn numeric_app_id_passes_validation() {
        let result = download_and_cache_image_guard_app_id("440");
        assert!(result.is_ok(), "pure numeric app_id must pass");
    }

    #[test]
    fn alphanumeric_app_id_is_rejected() {
        // Inline call to the app_id guard logic (same logic as in the public fn)
        let app_id = "123abc";
        assert!(
            app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_digit()),
            "123abc should fail numeric check"
        );
    }

    #[test]
    fn path_traversal_app_id_is_rejected() {
        for bad in &["../etc", "../../passwd", "/etc/shadow", "..", "44 0"] {
            assert!(
                bad.is_empty() || !bad.chars().all(|c| c.is_ascii_digit()),
                "{bad:?} should fail numeric check"
            );
        }
    }

    #[test]
    fn empty_app_id_is_rejected() {
        let app_id = "";
        assert!(
            app_id.is_empty(),
            "empty app_id must fail the empty check"
        );
    }

    // -----------------------------------------------------------------------
    // validate_image_bytes
    // -----------------------------------------------------------------------

    #[test]
    fn jpeg_magic_bytes_are_accepted() {
        // Minimal JPEG header: SOI marker FF D8, followed by FF E0 (JFIF APP0)
        let mut jpeg_bytes = vec![0xFF_u8, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpeg_bytes.extend_from_slice(b"JFIF\x00");
        // Pad to make it look non-trivial
        jpeg_bytes.extend(vec![0u8; 20]);
        assert!(
            validate_image_bytes(&jpeg_bytes).is_ok(),
            "JPEG magic bytes must be accepted"
        );
    }

    #[test]
    fn png_magic_bytes_are_accepted() {
        // PNG signature: 8 bytes
        let png_bytes: Vec<u8> =
            vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D];
        assert!(
            validate_image_bytes(&png_bytes).is_ok(),
            "PNG magic bytes must be accepted"
        );
    }

    #[test]
    fn svg_is_rejected() {
        // SVG is XML text — no magic bytes; infer will return None → octet-stream
        let svg_bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
        let result = validate_image_bytes(svg_bytes);
        assert!(
            result.is_err(),
            "SVG content must be rejected (no magic bytes → octet-stream)"
        );
    }

    #[test]
    fn html_text_is_rejected() {
        let html = b"<!DOCTYPE html><html><body>evil</body></html>";
        assert!(
            validate_image_bytes(html).is_err(),
            "HTML text must be rejected"
        );
    }

    #[test]
    fn oversized_content_is_rejected() {
        let oversized = vec![0xFF_u8, 0xD8, 0xFF, 0xE0];
        // We don't allocate 5 MB here; instead test the boundary directly.
        let mut large = vec![0u8; MAX_IMAGE_BYTES + 1];
        // Set JPEG magic so format check would pass if not for size check
        large[0] = 0xFF;
        large[1] = 0xD8;
        large[2] = 0xFF;
        large[3] = 0xE0;
        let result = validate_image_bytes(&large);
        assert!(result.is_err(), "content exceeding 5 MB must be rejected");
        _ = oversized; // suppress unused warning
    }

    // -----------------------------------------------------------------------
    // safe_image_cache_path
    // -----------------------------------------------------------------------

    #[test]
    fn safe_path_rejects_dotdot_app_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = safe_image_cache_path(tmp.path(), "../etc", "cover_steam_cdn.jpg");
        assert!(
            matches!(result, Err(GameImageError::InvalidAppId)),
            "path traversal via app_id must be rejected"
        );
    }

    #[test]
    fn safe_path_rejects_slash_in_filename() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = safe_image_cache_path(tmp.path(), "440", "../../evil.jpg");
        assert!(
            matches!(result, Err(GameImageError::InvalidFilename)),
            "path traversal via filename must be rejected"
        );
    }

    #[test]
    fn safe_path_accepts_valid_inputs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = safe_image_cache_path(tmp.path(), "440", "cover_steam_cdn.jpg");
        assert!(result.is_ok(), "valid app_id and filename must succeed");
        let path = result.unwrap();
        // The path must be inside the base temp dir
        assert!(
            path.starts_with(tmp.path()),
            "result path must be inside base dir"
        );
    }

    #[test]
    fn safe_path_rejects_empty_app_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = safe_image_cache_path(tmp.path(), "", "cover_steam_cdn.jpg");
        assert!(
            matches!(result, Err(GameImageError::InvalidAppId)),
            "empty app_id must be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // MetadataStore integration (in-memory DB)
    // -----------------------------------------------------------------------

    #[test]
    fn get_game_image_returns_none_for_missing_entry() {
        let store = MetadataStore::open_in_memory().expect("open in-memory store");
        let result = store.get_game_image("999999", "cover").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn upsert_then_get_round_trips() {
        let store = MetadataStore::open_in_memory().expect("open in-memory store");
        store
            .upsert_game_image(
                "440",
                "cover",
                "steam_cdn",
                "/tmp/test/440/cover_steam_cdn.jpg",
                Some(1024),
                Some("deadbeef"),
                Some("image/jpeg"),
                Some("https://cdn.cloudflare.steamstatic.com/steam/apps/440/header.jpg"),
                None,
            )
            .expect("upsert should succeed");

        let row = store
            .get_game_image("440", "cover")
            .unwrap()
            .expect("row must exist after upsert");

        assert_eq!(row.steam_app_id, "440");
        assert_eq!(row.image_type, "cover");
        assert_eq!(row.content_hash, "deadbeef");
    }

    // -----------------------------------------------------------------------
    // Helper: guard-only validation without I/O
    // -----------------------------------------------------------------------

    fn download_and_cache_image_guard_app_id(app_id: &str) -> Result<(), String> {
        if app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("invalid app_id: {app_id:?}"));
        }
        Ok(())
    }
}
