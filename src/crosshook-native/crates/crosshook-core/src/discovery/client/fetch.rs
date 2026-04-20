use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};

use crate::metadata::MetadataStore;

use super::parse::parse_rss_items;
use super::{
    DiscoveryError, RssItem, HTTP_CLIENT, MAX_CACHED_ITEMS, MAX_RESPONSE_BYTES,
    REQUEST_TIMEOUT_SECS,
};

/// Builds a WordPress search RSS URL from a source's base URL and query.
pub(super) fn wordpress_rss_search_url(
    base_url: &str,
    game_name: &str,
) -> Result<String, DiscoveryError> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|e| DiscoveryError::ParseError(format!("invalid base_url {base_url:?}: {e}")))?;
    url.query_pairs_mut()
        .append_pair("s", game_name.trim())
        .append_pair("feed", "rss2");
    Ok(url.into())
}

/// Dispatches URL construction based on source_type.
pub(super) fn build_fetch_url(
    source: &crate::discovery::models::ExternalTrainerSourceSubscription,
    game_name: &str,
) -> Result<String, DiscoveryError> {
    match source.source_type.as_str() {
        "wordpress_rss" => wordpress_rss_search_url(&source.base_url, game_name),
        other => Err(DiscoveryError::ParseError(format!(
            "unsupported source_type: {other}"
        ))),
    }
}

pub(super) async fn fetch_and_cache_search(
    metadata_store: &MetadataStore,
    url: &str,
    cache_key: &str,
) -> Result<Vec<RssItem>, DiscoveryError> {
    let client = http_client()?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(DiscoveryError::Network)?;

    // Content-Type validation: reject non-XML responses (captive portal mitigation).
    if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct = content_type.to_str().unwrap_or_default().to_lowercase();
        if !ct.contains("xml") && !ct.contains("rss") {
            return Err(DiscoveryError::ParseError(format!(
                "unexpected content-type: {ct}"
            )));
        }
    }

    // Response size guard.
    if let Some(len) = response.content_length() {
        if len as usize > MAX_RESPONSE_BYTES {
            return Err(DiscoveryError::ParseError(format!(
                "response body exceeds {MAX_RESPONSE_BYTES} byte limit ({len} bytes)"
            )));
        }
    }

    let body = response
        .error_for_status()
        .map_err(DiscoveryError::Network)?
        .text()
        .await
        .map_err(DiscoveryError::Network)?;

    if body.len() > MAX_RESPONSE_BYTES {
        return Err(DiscoveryError::ParseError(format!(
            "response body exceeds {MAX_RESPONSE_BYTES} byte limit ({} bytes)",
            body.len()
        )));
    }

    let mut items = parse_rss_items(&body)?;
    items.truncate(MAX_CACHED_ITEMS);

    // Persist to per-query cache.
    let expires_at = (Utc::now() + ChronoDuration::hours(super::CACHE_TTL_HOURS)).to_rfc3339();
    match serde_json::to_string(&items) {
        Ok(payload) => {
            if let Err(error) =
                metadata_store.put_cache_entry(url, cache_key, &payload, Some(&expires_at))
            {
                tracing::warn!(cache_key = %cache_key, %error, "failed to persist search cache");
            }
        }
        Err(error) => {
            tracing::warn!(cache_key = %cache_key, %error, "failed to serialize search results");
        }
    }

    Ok(items)
}

fn http_client() -> Result<&'static reqwest::Client, DiscoveryError> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(DiscoveryError::Network)?;

    let _ = HTTP_CLIENT.set(client);
    Ok(HTTP_CLIENT
        .get()
        .expect("HTTP client should be initialized before use"))
}
