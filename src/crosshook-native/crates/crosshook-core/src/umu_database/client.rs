use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use directories::BaseDirs;
use reqwest::{
    header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED},
    StatusCode,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::metadata::{sha256_hex, MetadataStore, MetadataStoreError};

const SOURCE_URL: &str =
    "https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv";
const CACHE_KEY: &str = "umu-database:csv";
/// Hours before a cached CSV entry is considered stale and a re-fetch is attempted.
/// 24 h balances freshness against network churn for a database that is updated infrequently.
const CACHE_TTL_HOURS: i64 = 24;
/// Hard connect+read timeout for the upstream CSV fetch.
/// 6 s is generous for a small CSV over a typical broadband connection but bounded
/// enough not to block the app startup path for too long on a flaky network.
const REQUEST_TIMEOUT_SECS: u64 = 6;
const MAX_CSV_BYTES: usize = 10 * 1_048_576; // 10 MiB hard cap — guards against unbounded body buffering

static UMU_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn source_url() -> String {
    // URL override for test isolation: enabled in debug builds (which covers
    // both `cargo test` unit tests and integration tests) but stripped from
    // release builds.  Only the env-var mechanism is kept — the OnceLock
    // test-setter was removed to eliminate the duplicate override path.
    #[cfg(any(test, debug_assertions))]
    {
        if let Ok(url) = std::env::var("CROSSHOOK_TEST_UMU_DATABASE_URL") {
            if !url.is_empty() {
                return url;
            }
        }
    }
    SOURCE_URL.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UmuDatabaseRefreshStatus {
    pub refreshed: bool,
    pub cached_at: Option<String>,
    pub source_url: String,
    pub reason: String,
}

#[derive(Debug)]
pub enum Error {
    Network(reqwest::Error),
    Io(std::io::Error),
    Metadata(String),
    Base(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(e) => write!(f, "network error fetching umu database: {e}"),
            Self::Io(e) => write!(f, "I/O error writing umu database: {e}"),
            Self::Metadata(msg) => write!(f, "umu database metadata error: {msg}"),
            Self::Base(msg) => write!(f, "umu database home directory error: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Network(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CachePayload {
    etag: Option<String>,
    last_modified: Option<String>,
    body_sha256: String,
    body_bytes: usize,
}

fn umu_http_client() -> Result<&'static reqwest::Client, Error> {
    if let Some(client) = UMU_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(Error::Network)?;

    let _ = UMU_HTTP_CLIENT.set(client);
    Ok(UMU_HTTP_CLIENT
        .get()
        .expect("umu database HTTP client should be initialized before use"))
}

fn csv_target_path() -> Result<std::path::PathBuf, Error> {
    let dirs = BaseDirs::new().ok_or_else(|| {
        Error::Base(
            "home directory not found — CrossHook requires a user home directory".to_string(),
        )
    })?;
    Ok(dirs
        .data_local_dir()
        .join(super::CROSSHOOK_UMU_DATABASE_CSV_SUBPATH))
}

/// Query the `external_cache_entries` row for the umu-database CSV regardless of expiry,
/// so we can still use the stored ETag/Last-Modified for conditional GET requests even
/// after the 24-hour TTL has elapsed.
fn load_cache_payload(store: &MetadataStore) -> Option<CachePayload> {
    store
        .with_sqlite_conn("load umu database cache metadata", |conn| {
            let json: Option<String> = conn
                .query_row(
                    "SELECT payload_json FROM external_cache_entries \
                     WHERE cache_key = ?1 LIMIT 1",
                    rusqlite::params![CACHE_KEY],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(|source| MetadataStoreError::Database {
                    action: "query umu database cache row",
                    source,
                })?
                .flatten();
            Ok(json)
        })
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<CachePayload>(&json).ok())
}

/// Fetch (or revalidate) the umu-launcher protonfix CSV from GitHub.
///
/// - Sends `If-None-Match` / `If-Modified-Since` headers when a prior ETag or
///   `Last-Modified` value exists in the metadata store.
/// - **304**: rotates `expires_at` in the cache; no disk write.
/// - **200**: atomic write to `~/.local/share/crosshook/umu-database.csv` via
///   `tempfile::NamedTempFile` (0600, unpredictable name) + `persist()` rename,
///   then upserts the cache metadata row.
/// - Network failure: warns and returns `Err` — existing cache and disk file are
///   left untouched.
/// - Metadata DB unavailable: CSV is still written to disk; `cached_at` is `None`.
pub async fn refresh_umu_database() -> Result<UmuDatabaseRefreshStatus, Error> {
    let url = source_url();
    let client = umu_http_client()?;

    // Load metadata store (best effort — failures degrade gracefully to no-metadata mode)
    let metadata_store = MetadataStore::try_new().ok();

    // Read the stored cache payload (etag, last_modified) for conditional request headers.
    // We query regardless of expiry so we can still send conditional headers after 24 h.
    let existing_payload: Option<CachePayload> =
        metadata_store.as_ref().and_then(load_cache_payload);

    // Build conditional GET request
    let mut request = client.get(&url);
    if let Some(ref payload) = existing_payload {
        if let Some(ref etag) = payload.etag {
            request = request.header(IF_NONE_MATCH, etag.as_str());
        }
        if let Some(ref last_modified) = payload.last_modified {
            request = request.header(IF_MODIFIED_SINCE, last_modified.as_str());
        }
    }

    let response = match request.send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "failed to fetch umu database");
            return Err(Error::Network(e));
        }
    };

    let now = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();

    // ── 304 Not Modified ─────────────────────────────────────────────────────
    if response.status() == StatusCode::NOT_MODIFIED {
        if let (Some(store), Some(ref payload)) = (&metadata_store, &existing_payload) {
            match serde_json::to_string(payload) {
                Ok(payload_json) => {
                    if let Err(e) =
                        store.put_cache_entry(&url, CACHE_KEY, &payload_json, Some(&expires_at))
                    {
                        tracing::warn!(
                            error = %e,
                            "failed to rotate umu database cache expiry on 304"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to serialize umu database cache payload on 304"
                    );
                }
            }
        }

        return Ok(UmuDatabaseRefreshStatus {
            refreshed: false,
            cached_at: Some(now),
            source_url: url,
            reason: "304 Not Modified".to_string(),
        });
    }

    // ── Error status ─────────────────────────────────────────────────────────
    let response = match response.error_for_status() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "umu database server returned an error status");
            return Err(Error::Network(e));
        }
    };

    // ── 200 OK ───────────────────────────────────────────────────────────────
    // Capture response headers before consuming the body.
    let new_etag = response
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let new_last_modified = response
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    // Guard against servers that declare a body larger than our hard cap before we
    // allocate anything.  Content-Length is advisory, so we also re-check after
    // buffering.
    if let Some(content_len) = response.content_length() {
        if content_len > MAX_CSV_BYTES as u64 {
            return Err(Error::Base(format!(
                "umu database response body too large: Content-Length {content_len} exceeds {MAX_CSV_BYTES} byte limit"
            )));
        }
    }

    let body = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "failed to read umu database response body");
            return Err(Error::Network(e));
        }
    };

    if body.len() > MAX_CSV_BYTES {
        return Err(Error::Base(format!(
            "umu database response body too large: {} bytes exceeds {MAX_CSV_BYTES} byte limit",
            body.len()
        )));
    }

    let body_sha256 = sha256_hex(&body);
    let body_bytes = body.len();

    // Atomic write: create a NamedTempFile in the target directory (unpredictable name,
    // 0600 permissions by default on Linux), write the body, then persist() atomically
    // renames it over the target path — no symlink-following, no fixed temp name.
    let target_path = csv_target_path()?;
    let parent = target_path.parent().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "csv target path has no parent directory",
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(Error::Io)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(Error::Io)?;
    std::io::Write::write_all(&mut tmp, &body).map_err(Error::Io)?;
    tmp.persist(&target_path).map_err(|e| Error::Io(e.error))?;

    // Upsert cache metadata row.
    let new_payload = CachePayload {
        etag: new_etag,
        last_modified: new_last_modified,
        body_sha256,
        body_bytes,
    };

    let cached_at = match &metadata_store {
        None => {
            tracing::warn!(
                "umu database metadata store unavailable — CSV written to disk but metadata not updated"
            );
            None
        }
        Some(store) => match serde_json::to_string(&new_payload) {
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize umu database cache payload");
                None
            }
            Ok(payload_json) => {
                match store.put_cache_entry(&url, CACHE_KEY, &payload_json, Some(&expires_at)) {
                    Ok(()) => Some(now),
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to persist umu database cache entry");
                        None
                    }
                }
            }
        },
    };

    let reason = if cached_at.is_some() {
        "fetched fresh copy".to_string()
    } else {
        "metadata db unavailable".to_string()
    };

    Ok(UmuDatabaseRefreshStatus {
        refreshed: true,
        cached_at,
        source_url: url,
        reason,
    })
}
