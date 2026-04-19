use std::sync::OnceLock;
use std::time::Duration;

use reqwest::redirect::Policy;

use crate::game_images::models::GameImageError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const REQUEST_TIMEOUT_SECS: u64 = 15;

/// Hosts to which the HTTP client is allowed to follow redirects (S-01, S-06).
///
/// Only `https://` redirects to these domains are followed; all others are
/// stopped to prevent SSRF and HTTP-downgrade attacks.
const ALLOWED_REDIRECT_HOSTS: &[&str] = &[
    "cdn.cloudflare.steamstatic.com",
    "steamcdn-a.akamaihd.net",
    "www.steamgriddb.com",
    "cdn2.steamgriddb.com",
];

static GAME_IMAGES_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

// ---------------------------------------------------------------------------
// HTTP client singleton
// ---------------------------------------------------------------------------

/// Returns true if the given host is in the redirect allow-list.
pub fn is_allowed_redirect_host(host: &str) -> bool {
    ALLOWED_REDIRECT_HOSTS.contains(&host)
}

pub fn http_client() -> Result<&'static reqwest::Client, GameImageError> {
    if let Some(client) = GAME_IMAGES_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .redirect(Policy::custom(|attempt| {
            let url = attempt.url();
            if url.scheme() != "https" {
                return attempt.stop();
            }
            if let Some(host) = url.host_str() {
                if is_allowed_redirect_host(host) {
                    return attempt.follow();
                }
            }
            attempt.stop()
        }))
        .build()
        .map_err(GameImageError::ClientBuild)?;

    let _ = GAME_IMAGES_HTTP_CLIENT.set(client);
    Ok(GAME_IMAGES_HTTP_CLIENT
        .get()
        .expect("game images HTTP client should be initialized before use"))
}
