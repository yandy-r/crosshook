//! Data-driven external trainer discovery client (Phase B).
//!
//! Searches configured external sources (stored in TOML settings) using a
//! 3-stage cache→live→stale-fallback pattern via `external_cache_entries`.
//! Source subscriptions drive the URL, parser, cache key, and display name —
//! nothing is hardcoded to a specific provider.

use std::fmt;
use std::sync::OnceLock;

mod cache;
mod fetch;
mod parse;
mod response;
mod search;
#[cfg(test)]
mod tests;

pub use search::search_external_trainers;

pub(super) const CACHE_TTL_HOURS: i64 = 1;
pub(super) const REQUEST_TIMEOUT_SECS: u64 = 10;
pub(super) const MAX_SOURCE_CONCURRENCY: usize = 4;
pub(super) const CACHE_NAMESPACE: &str = "trainer:source:v1";
pub(super) const MAX_CACHED_ITEMS: usize = 50;
pub(super) const MAX_RESPONSE_BYTES: usize = 1_048_576; // 1 MB

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
pub(super) struct CachedRssRow {
    pub(super) payload_json: String,
    pub(super) fetched_at: String,
    pub(super) _expires_at: Option<String>,
}

/// Raw RSS item parsed from a WordPress search feed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct RssItem {
    pub(super) title: String,
    pub(super) link: String,
    pub(super) pub_date: Option<String>,
}
