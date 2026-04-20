use tokio::task::JoinSet;

use crate::discovery::models::{
    ExternalTrainerResult, ExternalTrainerSearchQuery, ExternalTrainerSearchResponse,
    ExternalTrainerSourceSubscription,
};
use crate::metadata::MetadataStore;

use super::cache::{cache_key_for_source_query, load_cached_rss_row};
use super::fetch::{build_fetch_url, fetch_and_cache_search};
use super::response::{build_response_from_cache, build_response_from_items, offline_response};
use super::MAX_SOURCE_CONCURRENCY;

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
    let source_label = if enabled.len() == 1 {
        enabled[0].source_id.clone()
    } else {
        "multi".to_string()
    };
    let mut all_results: Vec<ExternalTrainerResult> = Vec::new();
    let mut any_offline = false;
    let mut all_offline = true;
    let mut any_cached = false;
    let mut any_stale = false;
    let mut first_cache_age: Option<i64> = None;

    let mut join_set: JoinSet<ExternalTrainerSearchResponse> = JoinSet::new();
    let mut in_flight = 0usize;
    let game_name_owned = game_name.to_string();

    for source in enabled.into_iter() {
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
