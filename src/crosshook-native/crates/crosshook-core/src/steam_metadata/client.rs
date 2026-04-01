use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::StatusCode;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;

use super::models::{
    cache_key_for_app_id, normalize_app_id, SteamAppDetails, SteamGenre,
    SteamMetadataLookupResult, SteamMetadataLookupState,
};
use crate::metadata::{MetadataStore, MetadataStoreError};

const STEAM_APPDETAILS_URL_BASE: &str = "https://store.steampowered.com/api/appdetails";
const CACHE_TTL_HOURS: i64 = 24;
const REQUEST_TIMEOUT_SECS: u64 = 6;
static STEAM_METADATA_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[derive(Debug)]
enum SteamMetadataError {
    NotFound,
    Network(reqwest::Error),
    InvalidAppId(String),
}

impl fmt::Display for SteamMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Steam app details not found for this App ID"),
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::InvalidAppId(id) => {
                write!(f, "app ID {id:?} is not a valid numeric Steam App ID")
            }
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CachedLookupRow {
    payload_json: String,
    fetched_at: String,
    expires_at: Option<String>,
}

// ── Steam API wire types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct SteamApiGenre {
    #[serde(default)]
    id: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SteamApiAppData {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    header_image: Option<String>,
    #[serde(default)]
    genres: Vec<SteamApiGenre>,
}

#[derive(Debug, Clone, Deserialize)]
struct SteamApiAppEntry {
    success: bool,
    #[serde(default)]
    data: Option<SteamApiAppData>,
}

// ── Public lookup function ──────────────────────────────────────────────────

pub async fn lookup_steam_metadata(
    store: &MetadataStore,
    app_id: &str,
    force_refresh: bool,
) -> SteamMetadataLookupResult {
    let Some(app_id) = normalize_app_id(app_id) else {
        return SteamMetadataLookupResult::default();
    };

    let cache_key = cache_key_for_app_id(&app_id)
        .expect("normalized app id must always produce a cache key");
    if !force_refresh {
        if let Some(valid_cache) = load_cached_lookup_row(store, &cache_key, false) {
            if let Some(result) = cached_result_from_row(&app_id, valid_cache, false) {
                return result;
            }
        }
    }

    match fetch_live_lookup(&app_id).await {
        Ok(mut result) => {
            attach_cache_metadata(&mut result, false, false);
            persist_lookup_result(store, &cache_key, &result);
            result
        }
        Err(error) => {
            tracing::warn!(app_id, %error, "Steam metadata live lookup failed");
            if let Some(stale_cache) = load_cached_lookup_row(store, &cache_key, true) {
                if let Some(result) = cached_result_from_row(&app_id, stale_cache, true) {
                    return result;
                }
            }

            SteamMetadataLookupResult {
                app_id,
                state: SteamMetadataLookupState::Unavailable,
                app_details: None,
                from_cache: false,
                is_stale: false,
            }
        }
    }
}

// ── Internal helpers ────────────────────────────────────────────────────────

fn steam_metadata_http_client() -> Result<&'static reqwest::Client, SteamMetadataError> {
    if let Some(client) = STEAM_METADATA_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(SteamMetadataError::Network)?;

    let _ = STEAM_METADATA_HTTP_CLIENT.set(client);
    Ok(STEAM_METADATA_HTTP_CLIENT
        .get()
        .expect("Steam metadata HTTP client should be initialized before use"))
}

async fn fetch_live_lookup(app_id: &str) -> Result<SteamMetadataLookupResult, SteamMetadataError> {
    let client = steam_metadata_http_client()?;
    let url = format!("{STEAM_APPDETAILS_URL_BASE}?appids={app_id}");

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(SteamMetadataError::Network)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(SteamMetadataError::NotFound);
    }

    let body: HashMap<String, SteamApiAppEntry> = response
        .error_for_status()
        .map_err(SteamMetadataError::Network)?
        .json()
        .await
        .map_err(SteamMetadataError::Network)?;

    let entry = body
        .get(app_id)
        .ok_or_else(|| SteamMetadataError::InvalidAppId(app_id.to_string()))?;

    if !entry.success {
        return Err(SteamMetadataError::NotFound);
    }

    let app_details = entry.data.as_ref().map(|data| SteamAppDetails {
        name: data.name.clone(),
        short_description: data.short_description.clone(),
        header_image: data.header_image.clone(),
        genres: data
            .genres
            .iter()
            .map(|g| SteamGenre {
                id: g.id.clone(),
                description: g.description.clone(),
            })
            .collect(),
    });

    Ok(SteamMetadataLookupResult {
        app_id: app_id.to_string(),
        state: SteamMetadataLookupState::Ready,
        app_details,
        from_cache: false,
        is_stale: false,
    })
}

fn attach_cache_metadata(result: &mut SteamMetadataLookupResult, from_cache: bool, is_stale: bool) {
    result.from_cache = from_cache;
    result.is_stale = is_stale;
}

fn persist_lookup_result(
    metadata_store: &MetadataStore,
    cache_key: &str,
    result: &SteamMetadataLookupResult,
) {
    let Ok(payload) = serde_json::to_string(result) else {
        tracing::warn!(cache_key, "failed to serialize Steam metadata cache payload");
        return;
    };

    let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();

    if let Err(error) = metadata_store.put_cache_entry(
        STEAM_APPDETAILS_URL_BASE,
        cache_key,
        &payload,
        Some(&expires_at),
    ) {
        tracing::warn!(cache_key, %error, "failed to persist Steam metadata cache payload");
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
        "load a cached Steam metadata row"
    } else {
        "load a valid cached Steam metadata row"
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
                action: "query a Steam metadata cache row",
                source,
            })
        })
        .ok()
        .flatten()
}

fn cached_result_from_row(
    app_id: &str,
    row: CachedLookupRow,
    is_stale: bool,
) -> Option<SteamMetadataLookupResult> {
    if row.payload_json.trim().is_empty() {
        return None;
    }

    let mut result =
        serde_json::from_str::<SteamMetadataLookupResult>(&row.payload_json).ok()?;

    result.app_id = app_id.to_string();
    result.state = if is_stale {
        SteamMetadataLookupState::Stale
    } else {
        SteamMetadataLookupState::Ready
    };
    result.from_cache = true;
    result.is_stale = is_stale;

    Some(result)
}
