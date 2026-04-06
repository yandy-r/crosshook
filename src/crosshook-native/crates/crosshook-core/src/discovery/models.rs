use serde::{Deserialize, Serialize};

/// Deserializes a `trainer-sources.json` manifest file from a community tap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSourcesManifest {
    pub schema_version: u32,
    pub game_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<u32>,
    pub sources: Vec<TrainerSourceEntry>,
}

/// Individual trainer source entry within a `TrainerSourcesManifest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSourceEntry {
    pub source_name: String,
    pub source_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Internal row type mapping to DB columns from `trainer_sources` joined with `community_taps`.
/// Not used at the IPC boundary.
#[derive(Debug, Clone)]
pub struct TrainerSourceRow {
    pub id: i64,
    pub tap_id: String,
    pub game_name: String,
    pub steam_app_id: Option<u32>,
    pub source_name: String,
    pub source_url: String,
    pub trainer_version: Option<String>,
    pub game_version: Option<String>,
    pub notes: Option<String>,
    pub sha256: Option<String>,
    pub relative_path: String,
    pub created_at: String,
    /// Sourced from JOIN with `community_taps`.
    pub tap_url: String,
}

/// IPC input from the frontend for trainer search queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchQuery {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Phase A search result shaped from a `trainer_sources` + `community_taps` JOIN.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchResult {
    pub id: i64,
    pub game_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<u32>,
    pub source_name: String,
    pub source_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub relative_path: String,
    pub tap_url: String,
    pub tap_local_path: String,
    pub relevance_score: f64,
}

/// IPC response wrapper for trainer search results.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchResponse {
    pub results: Vec<TrainerSearchResult>,
    pub total_count: i64,
}

/// Phase B: external trainer result from an RSS or API source (e.g. FLiNG).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTrainerResult {
    pub game_name: String,
    pub source_name: String,
    pub source_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<String>,
    pub source: String,
    pub relevance_score: f64,
}

/// Phase B: IPC input from the frontend for external trainer search queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTrainerSearchQuery {
    pub game_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_refresh: Option<bool>,
}

/// Phase B: IPC response wrapper for external trainer search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTrainerSearchResponse {
    pub results: Vec<ExternalTrainerResult>,
    pub source: String,
    pub cached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_age_secs: Option<i64>,
    pub is_stale: bool,
    pub offline: bool,
}

/// Phase B: cache state for external discovery lookups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryCacheState {
    Fresh,
    Stale,
    #[default]
    Unavailable,
}

/// Phase B: version match status between a trainer and the installed game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VersionMatchStatus {
    Exact,
    Compatible,
    NewerAvailable,
    Outdated,
    #[default]
    Unknown,
}

/// Phase B: version match result pairing a status with optional detail strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMatchResult {
    pub status: VersionMatchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_game_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_game_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
