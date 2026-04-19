use std::path::PathBuf;

use chrono::{Duration as ChronoDuration, Utc};

use crate::game_images::models::{GameImageError, GameImageSource, GameImageType};
use crate::game_images::steamgriddb::fetch_steamgriddb_image;
use crate::metadata::{sha256_hex, MetadataStore};

use super::cache::{
    delete_game_image_row, filename_for, image_cache_base_dir, parse_expiration,
    stale_fallback_path,
};
use super::download::{build_download_url, download_image_bytes, try_portrait_candidates};
use super::validation::{mime_extension, safe_image_cache_path, validate_image_bytes};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CACHE_TTL_HOURS: i64 = 24;

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
            .and_then(parse_expiration)
            .map(|expires_at| expires_at <= Utc::now())
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
                tracing::debug!(
                    app_id,
                    image_type = image_type_str,
                    "SteamGridDB image fetched"
                );
                (b, GameImageSource::SteamGridDb)
            }
            Err(GameImageError::AuthFailure { status, .. }) => {
                tracing::warn!(
                    "SteamGridDB auth failed (status {status}). Falling back to Steam CDN. Check your API key."
                );
                // Fall through to CDN download below
                if image_type == GameImageType::Portrait {
                    match try_portrait_candidates(app_id).await {
                        Ok(b) => (b, GameImageSource::SteamCdn),
                        Err(err) => {
                            tracing::warn!(app_id, image_type = image_type_str, %err, "all portrait CDN candidates failed");
                            return Ok(stale_fallback_path(store, app_id, &image_type_str));
                        }
                    }
                } else {
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
            Err(error) => {
                tracing::warn!(
                    app_id,
                    image_type = image_type_str,
                    %error,
                    "SteamGridDB fetch failed; falling back to Steam CDN"
                );
                // Fall back to Steam CDN
                if image_type == GameImageType::Portrait {
                    match try_portrait_candidates(app_id).await {
                        Ok(b) => (b, GameImageSource::SteamCdn),
                        Err(err) => {
                            tracing::warn!(app_id, image_type = image_type_str, %err, "all portrait CDN candidates failed");
                            return Ok(stale_fallback_path(store, app_id, &image_type_str));
                        }
                    }
                } else {
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
        }
    } else {
        // No API key — use Steam CDN directly
        if image_type == GameImageType::Portrait {
            match try_portrait_candidates(app_id).await {
                Ok(b) => (b, GameImageSource::SteamCdn),
                Err(err) => {
                    tracing::warn!(app_id, image_type = image_type_str, %err, "all portrait CDN candidates failed");
                    return Ok(stale_fallback_path(store, app_id, &image_type_str));
                }
            }
        } else {
            let cdn_url = build_download_url(app_id, image_type);
            match download_image_bytes(&cdn_url).await {
                Ok(b) => (b, GameImageSource::SteamCdn),
                Err(error) => {
                    tracing::warn!(app_id, image_type = image_type_str, %error, "game image download failed");
                    return Ok(stale_fallback_path(store, app_id, &image_type_str));
                }
            }
        }
    };

    // ------------------------------------------------------------------
    // Step (d): Validate image bytes (magic bytes, size, MIME allow-list)
    // ------------------------------------------------------------------
    let mime_type = match validate_image_bytes(&bytes) {
        Ok(mime_type) => mime_type,
        Err(error) => {
            tracing::warn!(
                app_id,
                image_type = image_type_str,
                %error,
                "game image rejected by validation"
            );
            return Ok(None);
        }
    };

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

    let filename = filename_for(image_type, source, mime_extension(mime_type));
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
        GameImageSource::SteamGridDb => {
            format!("https://www.steamgriddb.com/api/v2/grids/steam/{app_id}")
        }
    };
    let content_hash = sha256_hex(&bytes);
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
