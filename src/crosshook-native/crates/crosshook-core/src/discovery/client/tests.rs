use chrono::{Duration as ChronoDuration, Utc};

use crate::discovery::models::fling_default_source;
use crate::discovery::models::{ExternalTrainerSearchQuery, ExternalTrainerSourceSubscription};
use crate::metadata::MetadataStore;

use super::cache::{cache_key_for_source_query, load_cached_rss_row};
use super::fetch::wordpress_rss_search_url;
use super::parse::parse_rss_items;
use super::response::{build_response_from_cache, build_response_from_items};
use super::search_external_trainers;
use super::MAX_CACHED_ITEMS;

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
    let url = wordpress_rss_search_url("https://flingtrainer.com/", "Ghost of Tsushima").unwrap();
    assert_eq!(
        url,
        "https://flingtrainer.com/?s=Ghost+of+Tsushima&feed=rss2"
    );
}

#[test]
fn wordpress_rss_search_url_percent_encodes_reserved() {
    let url = wordpress_rss_search_url("https://flingtrainer.com/", "Doom & Quake=1#tag").unwrap();
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
    let items = vec![super::RssItem {
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

    let items = vec![super::RssItem {
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

    let items = vec![super::RssItem {
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
    let mut items: Vec<super::RssItem> = (0..100)
        .map(|i| super::RssItem {
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
