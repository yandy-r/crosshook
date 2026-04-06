//! Data-driven external trainer discovery client (Phase B).
//!
//! Searches configured external sources (stored in TOML settings) using a
//! 3-stage cache→live→stale-fallback pattern via `external_cache_entries`.
//! Source subscriptions drive the URL, parser, cache key, and display name —
//! nothing is hardcoded to a specific provider.

use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, OptionalExtension};
use tokio::task::JoinSet;

use super::matching;
use super::models::{
    ExternalTrainerResult, ExternalTrainerSearchQuery, ExternalTrainerSearchResponse,
    ExternalTrainerSourceSubscription,
};
use crate::metadata::{MetadataStore, MetadataStoreError};

const CACHE_TTL_HOURS: i64 = 1;
const REQUEST_TIMEOUT_SECS: u64 = 10;
const MAX_SOURCE_CONCURRENCY: usize = 4;
const CACHE_NAMESPACE: &str = "trainer:source:v1";
const MAX_CACHED_ITEMS: usize = 50;
const MAX_RESPONSE_BYTES: usize = 1_048_576; // 1 MB

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[derive(Debug)]
pub(crate) enum DiscoveryError {
    Network(reqwest::Error),
    ParseError(String),
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::ParseError(message) => write!(f, "parse error: {message}"),
        }
    }
}

#[derive(Debug, Clone)]
struct CachedRssRow {
    payload_json: String,
    fetched_at: String,
    _expires_at: Option<String>,
}

/// Raw RSS item parsed from a WordPress search feed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RssItem {
    title: String,
    link: String,
    pub_date: Option<String>,
}

/// Builds a per-source, per-query cache key.
/// Format: `trainer:source:v1:{source_id}:{normalized_query}`
fn cache_key_for_source_query(source_id: &str, game_name: &str) -> String {
    let normalized = game_name.trim().to_lowercase().replace(' ', "_");
    format!("{CACHE_NAMESPACE}:{source_id}:{normalized}")
}

/// Builds a WordPress search RSS URL from a source's base URL and query.
fn wordpress_rss_search_url(base_url: &str, game_name: &str) -> Result<String, DiscoveryError> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|e| DiscoveryError::ParseError(format!("invalid base_url {base_url:?}: {e}")))?;
    url.query_pairs_mut()
        .append_pair("s", game_name.trim())
        .append_pair("feed", "rss2");
    Ok(url.into())
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

/// Searches all enabled external sources for trainers matching the query.
///
/// Each source runs the 3-stage cache-first flow in parallel, up to a bounded
/// concurrency limit.
/// Results from all sources are merged and sorted by relevance.
pub async fn search_external_trainers(
    metadata_store: &MetadataStore,
    sources: &[ExternalTrainerSourceSubscription],
    query: &ExternalTrainerSearchQuery,
) -> ExternalTrainerSearchResponse {
    let game_name = query.game_name.trim();
    if game_name.is_empty() {
        return ExternalTrainerSearchResponse {
            results: vec![],
            source: String::new(),
            cached: false,
            cache_age_secs: None,
            is_stale: false,
            offline: false,
        };
    }

    let enabled: Vec<ExternalTrainerSourceSubscription> =
        sources.iter().filter(|s| s.enabled).cloned().collect();

    if enabled.is_empty() {
        return ExternalTrainerSearchResponse {
            results: vec![],
            source: String::new(),
            cached: false,
            cache_age_secs: None,
            is_stale: false,
            offline: false,
        };
    }

    let force_refresh = query.force_refresh.unwrap_or(false);
    let mut all_results: Vec<ExternalTrainerResult> = Vec::new();
    let mut any_offline = false;
    let mut all_offline = true;
    let mut any_cached = false;
    let mut any_stale = false;
    let mut first_cache_age: Option<i64> = None;

    let mut join_set: JoinSet<ExternalTrainerSearchResponse> = JoinSet::new();
    let mut in_flight = 0usize;
    let game_name_owned = game_name.to_string();

    for source in enabled.iter().cloned() {
        while in_flight >= MAX_SOURCE_CONCURRENCY {
            if let Some(joined) = join_set.join_next().await {
                in_flight -= 1;
                let response = match joined {
                    Ok(response) => response,
                    Err(error) => {
                        tracing::warn!(%error, "source search task failed");
                        continue;
                    }
                };
                if response.offline {
                    any_offline = true;
                } else {
                    all_offline = false;
                }
                if response.cached {
                    any_cached = true;
                    if first_cache_age.is_none() {
                        first_cache_age = response.cache_age_secs;
                    }
                }
                if response.is_stale {
                    any_stale = true;
                }
                all_results.extend(response.results);
            }
        }

        let metadata_store = metadata_store.clone();
        let game_name = game_name_owned.clone();
        join_set.spawn(async move {
            fetch_source(&metadata_store, &source, &game_name, force_refresh).await
        });
        in_flight += 1;
    }

    while let Some(joined) = join_set.join_next().await {
        let response = match joined {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(%error, "source search task failed");
                continue;
            }
        };
        if response.offline {
            any_offline = true;
        } else {
            all_offline = false;
        }
        if response.cached {
            any_cached = true;
            if first_cache_age.is_none() {
                first_cache_age = response.cache_age_secs;
            }
        }
        if response.is_stale {
            any_stale = true;
        }
        all_results.extend(response.results);
    }

    all_results.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.game_name.cmp(&b.game_name))
            .then_with(|| a.source_url.cmp(&b.source_url))
    });

    let source_label = if enabled.len() == 1 {
        enabled[0].source_id.clone()
    } else {
        "multi".to_string()
    };

    ExternalTrainerSearchResponse {
        results: all_results,
        source: source_label,
        cached: any_cached,
        cache_age_secs: first_cache_age,
        is_stale: any_stale,
        offline: all_offline && any_offline,
    }
}

/// Runs the 3-stage cache→live→stale flow for a single source.
async fn fetch_source(
    metadata_store: &MetadataStore,
    source: &ExternalTrainerSourceSubscription,
    game_name: &str,
    force_refresh: bool,
) -> ExternalTrainerSearchResponse {
    let key = cache_key_for_source_query(&source.source_id, game_name);

    // Stage 1: Check for valid (non-expired) per-query cache.
    if !force_refresh {
        if let Some(cached) = load_cached_rss_row(metadata_store, &key, false) {
            if let Some(response) = build_response_from_cache(&cached, source, game_name, false) {
                return response;
            }
        }
    }

    // Stage 2: Build URL and fetch live.
    let url = match build_fetch_url(source, game_name) {
        Ok(url) => url,
        Err(error) => {
            tracing::warn!(
                source_id = source.source_id,
                %error,
                "failed to build search URL"
            );
            return offline_response(source);
        }
    };

    match fetch_and_cache_search(metadata_store, &url, &key).await {
        Ok(items) => build_response_from_items(&items, source, game_name, false, false),
        Err(error) => {
            tracing::warn!(
                source_id = source.source_id,
                %error,
                "external search fetch failed"
            );

            // Stage 3: Stale fallback.
            if let Some(stale) = load_cached_rss_row(metadata_store, &key, true) {
                if let Some(response) = build_response_from_cache(&stale, source, game_name, true) {
                    return response;
                }
            }

            // Stage 4: Total failure.
            offline_response(source)
        }
    }
}

/// Dispatches URL construction based on source_type.
fn build_fetch_url(
    source: &ExternalTrainerSourceSubscription,
    game_name: &str,
) -> Result<String, DiscoveryError> {
    match source.source_type.as_str() {
        "wordpress_rss" => wordpress_rss_search_url(&source.base_url, game_name),
        other => Err(DiscoveryError::ParseError(format!(
            "unsupported source_type: {other}"
        ))),
    }
}

fn offline_response(source: &ExternalTrainerSourceSubscription) -> ExternalTrainerSearchResponse {
    ExternalTrainerSearchResponse {
        results: vec![],
        source: source.source_id.clone(),
        cached: false,
        cache_age_secs: None,
        is_stale: false,
        offline: true,
    }
}

async fn fetch_and_cache_search(
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
    let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();
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

fn parse_rss_items(xml: &str) -> Result<Vec<RssItem>, DiscoveryError> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut items = Vec::new();
    let mut in_item = false;
    let mut current_tag = String::new();
    let mut title = String::new();
    let mut link = String::new();
    let mut pub_date = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "item" {
                    in_item = true;
                    title.clear();
                    link.clear();
                    pub_date.clear();
                } else if in_item {
                    current_tag = tag_name;
                }
            }
            Ok(Event::Text(ref e)) if in_item => {
                let text = e.decode().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" => title.push_str(&text),
                    "link" => link.push_str(&text),
                    "pubDate" => pub_date.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "item" && in_item {
                    if !title.is_empty() && !link.is_empty() {
                        items.push(RssItem {
                            title: title.trim().to_string(),
                            link: link.trim().to_string(),
                            pub_date: if pub_date.trim().is_empty() {
                                None
                            } else {
                                Some(pub_date.trim().to_string())
                            },
                        });
                    }
                    in_item = false;
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(DiscoveryError::ParseError(format!(
                    "XML parse error at position {}: {error}",
                    reader.error_position()
                )));
            }
            _ => {}
        }
    }

    Ok(items)
}

fn load_cached_rss_row(
    metadata_store: &MetadataStore,
    key: &str,
    allow_expired: bool,
) -> Option<CachedRssRow> {
    if !metadata_store.is_available() {
        return None;
    }

    let now = Utc::now().to_rfc3339();
    let action = if allow_expired {
        "load a cached external search row"
    } else {
        "load a valid cached external search row"
    };

    metadata_store
        .with_sqlite_conn(action, |conn| {
            let sql = if allow_expired {
                "SELECT payload_json, fetched_at, expires_at \
                 FROM external_cache_entries WHERE cache_key = ?1 LIMIT 1"
            } else {
                "SELECT payload_json, fetched_at, expires_at \
                 FROM external_cache_entries \
                 WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2) LIMIT 1"
            };

            let row_params = if allow_expired {
                params![key]
            } else {
                params![key, now]
            };

            conn.query_row(sql, row_params, |row| {
                Ok(CachedRssRow {
                    payload_json: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    fetched_at: row.get::<_, String>(1)?,
                    _expires_at: row.get::<_, Option<String>>(2)?,
                })
            })
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "query an external search cache row",
                source,
            })
        })
        .ok()
        .flatten()
}

fn build_response_from_cache(
    row: &CachedRssRow,
    source: &ExternalTrainerSourceSubscription,
    game_name: &str,
    is_stale: bool,
) -> Option<ExternalTrainerSearchResponse> {
    if row.payload_json.trim().is_empty() {
        return None;
    }

    let items: Vec<RssItem> = serde_json::from_str(&row.payload_json).ok()?;
    let cache_age_secs = chrono::DateTime::parse_from_rfc3339(&row.fetched_at)
        .ok()
        .map(|fetched| (Utc::now() - fetched.with_timezone(&Utc)).num_seconds());

    Some(
        build_response_from_items(&items, source, game_name, true, is_stale)
            .with_cache_info(cache_age_secs),
    )
}

/// Converts parsed RSS items to the IPC response. The search endpoint already
/// filters by relevance, so we strip the "Trainer" suffix, score lightly for
/// ordering, and return all results.
fn build_response_from_items(
    items: &[RssItem],
    source: &ExternalTrainerSourceSubscription,
    game_name: &str,
    cached: bool,
    is_stale: bool,
) -> ExternalTrainerSearchResponse {
    let mut results: Vec<ExternalTrainerResult> = items
        .iter()
        .map(|item| {
            let stripped = matching::strip_trainer_suffix(&item.title);
            let score = matching::score_fling_result(game_name, &stripped);
            ExternalTrainerResult {
                game_name: stripped,
                source_name: source.display_name.clone(),
                source_url: item.link.clone(),
                pub_date: item.pub_date.clone(),
                source: source.source_id.clone(),
                relevance_score: score,
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    ExternalTrainerSearchResponse {
        results,
        source: source.source_id.clone(),
        cached,
        cache_age_secs: None,
        is_stale,
        offline: false,
    }
}

impl ExternalTrainerSearchResponse {
    fn with_cache_info(mut self, cache_age_secs: Option<i64>) -> Self {
        self.cache_age_secs = cache_age_secs;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::models::fling_default_source;

    fn test_store() -> MetadataStore {
        MetadataStore::open_in_memory().expect("in-memory store")
    }

    fn fling() -> ExternalTrainerSourceSubscription {
        fling_default_source()
    }

    #[test]
    fn cache_key_includes_source_id() {
        assert_eq!(
            cache_key_for_source_query("fling", "Ghost of Tsushima"),
            "trainer:source:v1:fling:ghost_of_tsushima"
        );
        assert_eq!(
            cache_key_for_source_query("other_site", "Elden Ring"),
            "trainer:source:v1:other_site:elden_ring"
        );
    }

    #[test]
    fn different_sources_get_different_keys() {
        let key_a = cache_key_for_source_query("fling", "Elden Ring");
        let key_b = cache_key_for_source_query("other", "Elden Ring");
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn wordpress_rss_search_url_encodes_query() {
        let url =
            wordpress_rss_search_url("https://flingtrainer.com/", "Ghost of Tsushima").unwrap();
        assert_eq!(
            url,
            "https://flingtrainer.com/?s=Ghost+of+Tsushima&feed=rss2"
        );
    }

    #[test]
    fn wordpress_rss_search_url_percent_encodes_reserved() {
        let url =
            wordpress_rss_search_url("https://flingtrainer.com/", "Doom & Quake=1#tag").unwrap();
        assert_eq!(
            url,
            "https://flingtrainer.com/?s=Doom+%26+Quake%3D1%23tag&feed=rss2"
        );
    }

    #[test]
    fn parse_rss_items_extracts_items() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Search results</title>
    <item>
      <title>Ghost of Tsushima Trainer</title>
      <link>https://flingtrainer.com/trainer/ghost-of-tsushima-trainer/</link>
      <pubDate>Mon, 20 May 2024 00:55:38 +0000</pubDate>
    </item>
  </channel>
</rss>"#;

        let items = parse_rss_items(xml).expect("should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Ghost of Tsushima Trainer");
        assert!(items[0].pub_date.is_some());
    }

    #[test]
    fn parse_rss_items_handles_empty_feed() {
        let xml = r#"<?xml version="1.0"?><rss><channel></channel></rss>"#;
        let items = parse_rss_items(xml).expect("should parse");
        assert!(items.is_empty());
    }

    #[test]
    fn build_response_uses_source_fields() {
        let source = ExternalTrainerSourceSubscription {
            source_id: "my_source".to_string(),
            display_name: "My Source".to_string(),
            base_url: "https://example.com/".to_string(),
            source_type: "wordpress_rss".to_string(),
            enabled: true,
        };
        let items = vec![RssItem {
            title: "Elden Ring Trainer".into(),
            link: "https://example.com/elden".into(),
            pub_date: None,
        }];

        let response = build_response_from_items(&items, &source, "Elden Ring", false, false);
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].source_name, "My Source");
        assert_eq!(response.results[0].source, "my_source");
        assert_eq!(response.source, "my_source");
    }

    #[tokio::test]
    async fn cache_hit_returns_without_http_fetch() {
        let store = test_store();
        let source = fling();
        let key = cache_key_for_source_query(&source.source_id, "Ghost of Tsushima");
        let url = wordpress_rss_search_url(&source.base_url, "Ghost of Tsushima").unwrap();

        let items = vec![RssItem {
            title: "Ghost of Tsushima Trainer".into(),
            link: "https://flingtrainer.com/trainer/ghost-of-tsushima-trainer/".into(),
            pub_date: Some("Mon, 20 May 2024 00:55:38 +0000".into()),
        }];
        let payload = serde_json::to_string(&items).unwrap();
        let expires = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();

        store
            .put_cache_entry(&url, &key, &payload, Some(&expires))
            .expect("put cache");

        let query = ExternalTrainerSearchQuery {
            game_name: "Ghost of Tsushima".into(),
            steam_app_id: None,
            force_refresh: None,
        };

        let result = search_external_trainers(&store, &[source], &query).await;
        assert!(result.cached);
        assert!(!result.is_stale);
        assert!(!result.offline);
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].game_name, "Ghost of Tsushima");
        assert_eq!(result.results[0].source_name, "FLiNG");
        assert_eq!(result.results[0].source, "fling");
    }

    #[test]
    fn disabled_store_returns_no_cached_row() {
        let store = MetadataStore::disabled();
        let key = cache_key_for_source_query("fling", "test");
        assert!(load_cached_rss_row(&store, &key, false).is_none());
        assert!(load_cached_rss_row(&store, &key, true).is_none());
    }

    #[test]
    fn stale_cache_row_returns_stale_response() {
        let store = test_store();
        let source = fling();
        let key = cache_key_for_source_query(&source.source_id, "Elden Ring");
        let url = wordpress_rss_search_url(&source.base_url, "Elden Ring").unwrap();

        let items = vec![RssItem {
            title: "Elden Ring Trainer".into(),
            link: "https://flingtrainer.com/trainer/elden-ring/".into(),
            pub_date: None,
        }];
        let payload = serde_json::to_string(&items).unwrap();
        let expired = (Utc::now() - ChronoDuration::hours(2)).to_rfc3339();

        store
            .put_cache_entry(&url, &key, &payload, Some(&expired))
            .expect("put cache");

        assert!(load_cached_rss_row(&store, &key, false).is_none());

        let stale_row = load_cached_rss_row(&store, &key, true);
        assert!(stale_row.is_some());

        let response = build_response_from_cache(&stale_row.unwrap(), &source, "Elden Ring", true);
        assert!(response.is_some());
        let response = response.unwrap();
        assert!(response.is_stale);
        assert!(response.cached);
        assert_eq!(response.results.len(), 1);
    }

    #[tokio::test]
    async fn empty_sources_returns_empty() {
        let store = test_store();
        let query = ExternalTrainerSearchQuery {
            game_name: "Elden Ring".into(),
            steam_app_id: None,
            force_refresh: None,
        };
        let result = search_external_trainers(&store, &[], &query).await;
        assert!(result.results.is_empty());
        assert!(!result.offline);
    }

    #[tokio::test]
    async fn disabled_source_is_skipped() {
        let store = test_store();
        let mut source = fling();
        source.enabled = false;

        let query = ExternalTrainerSearchQuery {
            game_name: "Elden Ring".into(),
            steam_app_id: None,
            force_refresh: None,
        };
        let result = search_external_trainers(&store, &[source], &query).await;
        assert!(result.results.is_empty());
        assert!(!result.offline);
    }

    #[test]
    fn truncation_keeps_items_under_limit() {
        let mut items: Vec<RssItem> = (0..100)
            .map(|i| RssItem {
                title: format!("Game {i} Trainer"),
                link: format!("https://example.com/{i}"),
                pub_date: None,
            })
            .collect();

        items.truncate(MAX_CACHED_ITEMS);
        assert_eq!(items.len(), MAX_CACHED_ITEMS);

        let payload = serde_json::to_string(&items).unwrap();
        assert!(
            payload.len() < crate::metadata::MAX_CACHE_PAYLOAD_BYTES,
            "serialized {} items should be under 512 KiB, got {} bytes",
            MAX_CACHED_ITEMS,
            payload.len()
        );
    }
}
