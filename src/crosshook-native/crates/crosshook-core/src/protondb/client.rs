use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use reqwest::StatusCode;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;

use super::aggregation::{
    degraded_recommendation_group, normalize_report_feed, ProtonDbReportFeedResponse,
};
use super::models::{
    cache_key_for_app_id, normalize_app_id, ProtonDbCacheState, ProtonDbLookupResult,
    ProtonDbLookupState, ProtonDbSnapshot, ProtonDbTier,
};
use crate::metadata::{MetadataStore, MetadataStoreError};

const APP_PAGE_URL_BASE: &str = "https://www.protondb.com/app";
const COUNTS_URL: &str = "https://www.protondb.com/data/counts.json";
const REPORTS_URL_BASE: &str = "https://www.protondb.com/data/reports";
const SUMMARY_URL_BASE: &str = "https://www.protondb.com/api/v1/reports/summaries";
const CACHE_TTL_HOURS: i64 = 6;
const PAGE_SELECTOR_FIRST: i64 = 1;
const REQUEST_TIMEOUT_SECS: u64 = 6;
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[derive(Debug)]
enum ProtonDbError {
    NotFound,
    HashResolutionFailed,
    Network(reqwest::Error),
    InvalidAppId(String),
    InvalidTimestamp(i64),
}

impl fmt::Display for ProtonDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "ProtonDB summary not found for this Steam App ID"),
            Self::HashResolutionFailed => {
                write!(f, "ProtonDB report feed hash could not be resolved")
            }
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::InvalidAppId(id) => {
                write!(f, "app ID {id:?} cannot be used for a report feed lookup")
            }
            Self::InvalidTimestamp(ts) => {
                write!(f, "ProtonDB counts timestamp {ts} is not positive")
            }
        }
    }
}

#[derive(Debug, Clone)]
struct CachedLookupRow {
    payload_json: String,
    fetched_at: String,
    expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtonDbSummaryResponse {
    #[serde(default)]
    tier: ProtonDbTier,
    #[serde(default)]
    best_reported_tier: Option<ProtonDbTier>,
    #[serde(default)]
    trending_tier: Option<ProtonDbTier>,
    #[serde(default)]
    score: Option<f32>,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default, rename = "total")]
    total_reports: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProtonDbCountsResponse {
    reports: i64,
    timestamp: i64,
}

pub async fn lookup_protondb(
    metadata_store: &MetadataStore,
    app_id: &str,
    force_refresh: bool,
) -> ProtonDbLookupResult {
    let Some(app_id) = normalize_app_id(app_id) else {
        return ProtonDbLookupResult::default();
    };

    let cache_key = cache_key_for_app_id(&app_id);
    if !force_refresh {
        if let Some(valid_cache) = load_cached_lookup_row(metadata_store, &cache_key, false) {
            if let Some(result) = cached_result_from_row(&app_id, &cache_key, valid_cache, false) {
                return result;
            }
        }
    }

    match fetch_live_lookup(&app_id).await {
        Ok(mut result) => {
            attach_cache_state(&mut result, &cache_key, false, false);
            persist_lookup_result(metadata_store, &cache_key, &result);
            result
        }
        Err(error) => {
            tracing::warn!(app_id, %error, "ProtonDB live lookup failed");
            if let Some(stale_cache) = load_cached_lookup_row(metadata_store, &cache_key, true) {
                if let Some(result) = cached_result_from_row(&app_id, &cache_key, stale_cache, true)
                {
                    return result;
                }
            }

            ProtonDbLookupResult {
                app_id,
                state: ProtonDbLookupState::Unavailable,
                cache: Some(ProtonDbCacheState {
                    cache_key,
                    is_offline: true,
                    ..ProtonDbCacheState::default()
                }),
                snapshot: None,
            }
        }
    }
}

async fn fetch_live_lookup(app_id: &str) -> Result<ProtonDbLookupResult, ProtonDbError> {
    let client = protondb_http_client()?;
    let summary_url = format!("{SUMMARY_URL_BASE}/{app_id}.json");
    let summary = fetch_summary(client, &summary_url).await?;
    let fetched_at = Utc::now().to_rfc3339();

    let mut snapshot = ProtonDbSnapshot {
        app_id: app_id.to_string(),
        tier: summary.tier,
        best_reported_tier: summary.best_reported_tier,
        trending_tier: summary.trending_tier,
        score: summary.score,
        confidence: summary.confidence,
        total_reports: summary.total_reports,
        source_url: format!("{APP_PAGE_URL_BASE}/{app_id}"),
        fetched_at,
        ..ProtonDbSnapshot::default()
    };

    match fetch_recommendations(client, app_id).await {
        Ok(recommendation_groups) => {
            snapshot.recommendation_groups = recommendation_groups;
        }
        Err(error) => {
            tracing::warn!(app_id, %error, "ProtonDB report aggregation degraded");
            let message = match error {
                ProtonDbError::HashResolutionFailed | ProtonDbError::InvalidAppId(_) => {
                    "No community report data is available for this game on ProtonDB yet."
                }
                _ => "Community report data could not be loaded right now. Tier information is still shown above.",
            };
            snapshot.recommendation_groups = vec![degraded_recommendation_group(message)];
        }
    }

    Ok(ProtonDbLookupResult {
        app_id: app_id.to_string(),
        state: ProtonDbLookupState::Ready,
        cache: None,
        snapshot: Some(snapshot),
    })
}

fn protondb_http_client() -> Result<&'static reqwest::Client, ProtonDbError> {
    if let Some(client) = PROTONDB_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(ProtonDbError::Network)?;

    let _ = PROTONDB_HTTP_CLIENT.set(client);
    Ok(PROTONDB_HTTP_CLIENT
        .get()
        .expect("ProtonDB HTTP client should be initialized before use"))
}

async fn fetch_summary(
    client: &reqwest::Client,
    summary_url: &str,
) -> Result<ProtonDbSummaryResponse, ProtonDbError> {
    let response = client
        .get(summary_url)
        .send()
        .await
        .map_err(ProtonDbError::Network)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(ProtonDbError::NotFound);
    }

    response
        .error_for_status()
        .map_err(ProtonDbError::Network)?
        .json::<ProtonDbSummaryResponse>()
        .await
        .map_err(ProtonDbError::Network)
}

async fn fetch_counts_json(
    client: &reqwest::Client,
) -> Result<ProtonDbCountsResponse, ProtonDbError> {
    client
        .get(COUNTS_URL)
        .send()
        .await
        .map_err(ProtonDbError::Network)?
        .error_for_status()
        .map_err(ProtonDbError::Network)?
        .json::<ProtonDbCountsResponse>()
        .await
        .map_err(ProtonDbError::Network)
}

async fn fetch_recommendations(
    client: &reqwest::Client,
    app_id: &str,
) -> Result<Vec<super::models::ProtonDbRecommendationGroup>, ProtonDbError> {
    let mut counts = fetch_counts_json(client).await?;

    if counts.timestamp <= 0 {
        return Err(ProtonDbError::InvalidTimestamp(counts.timestamp));
    }

    let app_id_i64 = app_id
        .parse::<i64>()
        .map_err(|_| ProtonDbError::InvalidAppId(app_id.to_string()))?;

    let mut report_feed_url = format!(
        "{REPORTS_URL_BASE}/all-devices/app/{}.json",
        report_feed_id(
            app_id_i64,
            counts.reports,
            counts.timestamp,
            PAGE_SELECTOR_FIRST,
        )
    );

    let mut response = client
        .get(&report_feed_url)
        .send()
        .await
        .map_err(ProtonDbError::Network)?;

    if response.status() == StatusCode::NOT_FOUND {
        counts = fetch_counts_json(client).await?;
        if counts.timestamp <= 0 {
            return Err(ProtonDbError::InvalidTimestamp(counts.timestamp));
        }
        report_feed_url = format!(
            "{REPORTS_URL_BASE}/all-devices/app/{}.json",
            report_feed_id(
                app_id_i64,
                counts.reports,
                counts.timestamp,
                PAGE_SELECTOR_FIRST,
            )
        );
        response = client
            .get(&report_feed_url)
            .send()
            .await
            .map_err(ProtonDbError::Network)?;
    }

    if response.status() == StatusCode::NOT_FOUND {
        return Err(ProtonDbError::HashResolutionFailed);
    }

    let feed = response
        .error_for_status()
        .map_err(ProtonDbError::Network)?
        .json::<ProtonDbReportFeedResponse>()
        .await
        .map_err(ProtonDbError::Network)?;

    Ok(normalize_report_feed(feed))
}

fn attach_cache_state(
    result: &mut ProtonDbLookupResult,
    cache_key: &str,
    from_cache: bool,
    is_stale: bool,
) {
    let fetched_at = result
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.fetched_at.clone())
        .unwrap_or_default();
    let expires_at = parse_rfc3339(&fetched_at)
        .map(|time| (time + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339());

    result.cache = Some(ProtonDbCacheState {
        cache_key: cache_key.to_string(),
        fetched_at,
        expires_at,
        from_cache,
        is_stale,
        is_offline: from_cache,
    });
}

fn persist_lookup_result(
    metadata_store: &MetadataStore,
    cache_key: &str,
    result: &ProtonDbLookupResult,
) {
    let Some(cache) = result.cache.as_ref() else {
        return;
    };

    let Ok(payload) = serde_json::to_string(result) else {
        tracing::warn!(cache_key, "failed to serialize ProtonDB cache payload");
        return;
    };

    if let Err(error) = metadata_store.put_cache_entry(
        result
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.source_url.as_str())
            .unwrap_or_default(),
        cache_key,
        &payload,
        cache.expires_at.as_deref(),
    ) {
        tracing::warn!(cache_key, %error, "failed to persist ProtonDB cache payload");
    }
}

fn load_cached_lookup_row(
    metadata_store: &MetadataStore,
    cache_key: &str,
    allow_expired: bool,
) -> Option<CachedLookupRow> {
    if !metadata_store.is_available() {
        return None;
    }

    let now = Utc::now().to_rfc3339();
    let action = if allow_expired {
        "load a cached ProtonDB row"
    } else {
        "load a valid cached ProtonDB row"
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
                params![cache_key]
            } else {
                params![cache_key, now]
            };

            conn.query_row(sql, row_params, |row| {
                Ok(CachedLookupRow {
                    payload_json: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    fetched_at: row.get::<_, String>(1)?,
                    expires_at: row.get::<_, Option<String>>(2)?,
                })
            })
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "query a ProtonDB cache row",
                source,
            })
        })
        .ok()
        .flatten()
}

fn cached_result_from_row(
    app_id: &str,
    cache_key: &str,
    row: CachedLookupRow,
    is_stale: bool,
) -> Option<ProtonDbLookupResult> {
    if row.payload_json.trim().is_empty() {
        return None;
    }

    let mut result = serde_json::from_str::<ProtonDbLookupResult>(&row.payload_json).ok()?;
    if result.snapshot.is_none() {
        return None;
    }

    result.app_id = app_id.to_string();
    result.state = if is_stale {
        ProtonDbLookupState::Stale
    } else {
        ProtonDbLookupState::Ready
    };
    result.cache = Some(ProtonDbCacheState {
        cache_key: cache_key.to_string(),
        fetched_at: row.fetched_at.clone(),
        expires_at: row.expires_at.clone(),
        from_cache: true,
        is_stale,
        is_offline: is_stale,
    });

    if let Some(snapshot) = result.snapshot.as_mut() {
        if snapshot.app_id.trim().is_empty() {
            snapshot.app_id = app_id.to_string();
        }
        if snapshot.fetched_at.trim().is_empty() {
            snapshot.fetched_at = row.fetched_at;
        }
    }

    Some(result)
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn report_feed_id(
    app_id: i64,
    reports_count: i64,
    counts_timestamp: i64,
    page_selector: i64,
) -> i64 {
    hash_text(format!(
        "p{}*vRT{}",
        compose_hash_part(app_id, reports_count, counts_timestamp),
        compose_hash_part(page_selector, app_id, counts_timestamp)
    ))
}

fn compose_hash_part(left: i64, right: i64, modulus: i64) -> String {
    format!("{right}p{}", left * (right % modulus))
}

fn hash_text(value: String) -> i64 {
    value
        .chars()
        .chain(std::iter::once('m'))
        .fold(0_i32, |acc, ch| {
            acc.wrapping_mul(31).wrapping_add(ch as i32)
        })
        .unsigned_abs() as i64
}
