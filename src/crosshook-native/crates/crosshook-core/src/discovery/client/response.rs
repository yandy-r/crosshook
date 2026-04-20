use chrono::Utc;

use crate::discovery::matching;
use crate::discovery::models::{
    ExternalTrainerResult, ExternalTrainerSearchResponse, ExternalTrainerSourceSubscription,
};

use super::{CachedRssRow, RssItem};

pub(super) fn offline_response(
    source: &ExternalTrainerSourceSubscription,
) -> ExternalTrainerSearchResponse {
    ExternalTrainerSearchResponse {
        results: vec![],
        source: source.source_id.clone(),
        cached: false,
        cache_age_secs: None,
        is_stale: false,
        offline: true,
    }
}

pub(super) fn build_response_from_cache(
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
pub(super) fn build_response_from_items(
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
