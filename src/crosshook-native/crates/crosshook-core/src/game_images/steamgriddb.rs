use serde::Deserialize;

use super::client::{http_client, read_limited_response};
use super::models::{GameImageError, GameImageType};

// ---------------------------------------------------------------------------
// SteamGridDB API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SteamGridDbResponse {
    success: bool,
    data: Option<Vec<SteamGridDbItem>>,
}

#[derive(Debug, Deserialize)]
struct SteamGridDbItem {
    url: String,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MB

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetch a game image from SteamGridDB for the given Steam `app_id`.
///
/// Uses Bearer token authentication with the provided `api_key`.  The API key
/// is deliberately excluded from tracing spans via `skip(api_key)` to prevent
/// accidental logging.
///
/// Returns the raw image bytes on success.
#[tracing::instrument(skip(api_key), fields(app_id, image_type = %image_type))]
pub async fn fetch_steamgriddb_image(
    api_key: &str,
    app_id: &str,
    image_type: &GameImageType,
) -> Result<Vec<u8>, GameImageError> {
    let endpoint = build_endpoint(app_id, image_type);

    tracing::debug!(app_id, endpoint, "fetching image from SteamGridDB");

    let client = http_client()?;
    let raw_response = client
        .get(&endpoint)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(GameImageError::Network)?;

    let status_code = raw_response.status();
    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        return Err(GameImageError::AuthFailure {
            status: status_code.as_u16(),
            message: "SteamGridDB API key is missing, invalid, or expired".to_string(),
        });
    }

    let response = raw_response
        .error_for_status()
        .map_err(GameImageError::Network)?;

    let body: SteamGridDbResponse = response.json().await.map_err(GameImageError::Network)?;

    if !body.success {
        return Err(GameImageError::Store(
            "SteamGridDB API returned success=false".to_string(),
        ));
    }

    let items = body.data.unwrap_or_default();
    let first_url = items
        .into_iter()
        .next()
        .map(|item| item.url)
        .ok_or_else(|| {
            GameImageError::Store("SteamGridDB returned no items for this app".to_string())
        })?;

    tracing::debug!(app_id, image_url = %first_url, "downloading SteamGridDB image");

    let image_response = client
        .get(&first_url)
        .send()
        .await
        .map_err(GameImageError::Network)?
        .error_for_status()
        .map_err(GameImageError::Network)?;

    if image_response
        .content_length()
        .is_some_and(|content_length| content_length > MAX_IMAGE_BYTES as u64)
    {
        return Err(GameImageError::TooLarge);
    }

    read_limited_response(image_response).await
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn build_endpoint(app_id: &str, image_type: &GameImageType) -> String {
    let (path_segment, dimensions) = match image_type {
        GameImageType::Cover => ("grids", Some("460x215,920x430")),
        GameImageType::Hero => ("heroes", None),
        GameImageType::Capsule => ("grids", Some("342x482,600x900")),
        GameImageType::Portrait => ("grids", Some("342x482,600x900")),
        GameImageType::Background => ("heroes", None),
    };

    match dimensions {
        Some(dimensions) => {
            format!(
                "https://www.steamgriddb.com/api/v2/{path_segment}/steam/{app_id}?dimensions={dimensions}"
            )
        }
        None => format!("https://www.steamgriddb.com/api/v2/{path_segment}/steam/{app_id}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_endpoint_cover_uses_grids() {
        let url = build_endpoint("440", &GameImageType::Cover);
        assert_eq!(
            url,
            "https://www.steamgriddb.com/api/v2/grids/steam/440?dimensions=460x215,920x430"
        );
    }

    #[test]
    fn build_endpoint_hero_uses_heroes() {
        let url = build_endpoint("440", &GameImageType::Hero);
        assert_eq!(url, "https://www.steamgriddb.com/api/v2/heroes/steam/440");
    }

    #[test]
    fn build_endpoint_capsule_uses_grids() {
        let url = build_endpoint("440", &GameImageType::Capsule);
        assert_eq!(
            url,
            "https://www.steamgriddb.com/api/v2/grids/steam/440?dimensions=342x482,600x900"
        );
    }

    #[test]
    fn build_endpoint_portrait_uses_grids() {
        let url = build_endpoint("440", &GameImageType::Portrait);
        assert_eq!(
            url,
            "https://www.steamgriddb.com/api/v2/grids/steam/440?dimensions=342x482,600x900"
        );
    }

    #[test]
    fn build_endpoint_background_uses_heroes() {
        let url = build_endpoint("440", &GameImageType::Background);
        assert_eq!(url, "https://www.steamgriddb.com/api/v2/heroes/steam/440");
    }
}
