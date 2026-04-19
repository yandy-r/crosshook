use crate::game_images::models::{GameImageError, GameImageType};

use super::http::http_client;
use super::validation::validate_image_bytes;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MB

// ---------------------------------------------------------------------------
// Download helpers
// ---------------------------------------------------------------------------

pub(super) fn build_download_url(app_id: &str, image_type: GameImageType) -> String {
    match image_type {
        GameImageType::Cover => {
            format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg")
        }
        GameImageType::Hero => {
            format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero.jpg")
        }
        GameImageType::Capsule => {
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/capsule_616x353.jpg"
            )
        }
        GameImageType::Portrait => {
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
            )
        }
        // Background uses same CDN file as Hero (library_hero.jpg, 3840x1240)
        GameImageType::Background => {
            format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero.jpg")
        }
    }
}

pub(super) fn portrait_candidate_urls(app_id: &str) -> Vec<String> {
    vec![
        format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
        ),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"),
    ]
}

/// Try each portrait candidate URL in order, returning the bytes from the
/// first successful download or the last error if all candidates fail.
pub(super) async fn try_portrait_candidates(app_id: &str) -> Result<Vec<u8>, GameImageError> {
    let candidates = portrait_candidate_urls(app_id);
    let mut last_err = None;
    for url in &candidates {
        match download_image_bytes(url).await {
            Ok(bytes) => {
                // Validate the downloaded bytes before returning
                validate_image_bytes(&bytes)?;
                return Ok(bytes);
            }
            Err(e) => last_err = Some(e),
        }
    }
    // portrait_candidate_urls always returns at least one URL.
    Err(last_err.expect("portrait_candidate_urls returned no candidates"))
}

pub(super) async fn download_image_bytes(url: &str) -> Result<Vec<u8>, GameImageError> {
    let client = http_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(GameImageError::Network)?
        .error_for_status()
        .map_err(GameImageError::Network)?;

    read_limited_response(response).await
}

pub async fn read_limited_response(
    mut response: reqwest::Response,
) -> Result<Vec<u8>, GameImageError> {
    if let Some(content_length) = response.content_length() {
        if content_length > MAX_IMAGE_BYTES as u64 {
            return Err(GameImageError::TooLarge);
        }
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(GameImageError::Network)? {
        if bytes.len() + chunk.len() > MAX_IMAGE_BYTES {
            return Err(GameImageError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}
