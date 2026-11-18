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

/// External trainer result from an RSS or API source (e.g. FLiNG).
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

/// IPC input from the frontend for external trainer search queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTrainerSearchQuery {
    pub game_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_refresh: Option<bool>,
}

/// IPC response wrapper for external trainer search results.
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

/// Version match status between a trainer and the installed game.
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

/// Version match result pairing a status with optional detail strings.
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

// ---------------------------------------------------------------------------
// External trainer source subscriptions (data-driven, stored in TOML settings)
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// A user-managed subscription to an external trainer discovery source.
/// Stored in `settings.toml` under `[[external_trainer_sources]]`.
/// Analogous to `CommunityTapSubscription` for community taps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTrainerSourceSubscription {
    /// Stable machine identifier (e.g. `"fling"`). Used in cache keys and IPC.
    pub source_id: String,
    /// Human-readable display name (e.g. `"FLiNG"`). Shown in UI badges.
    pub display_name: String,
    /// Base URL for the search endpoint (e.g. `"https://flingtrainer.com/"`).
    /// Must be HTTPS.
    pub base_url: String,
    /// Parser/protocol variant. Currently only `"wordpress_rss"`.
    pub source_type: String,
    /// Whether this source participates in searches.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Unknown TOML fields retained for forward-compatible local persistence.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

/// Returns the built-in FLiNG default source subscription.
pub fn fling_default_source() -> ExternalTrainerSourceSubscription {
    ExternalTrainerSourceSubscription {
        extra: toml::Table::new(),
        source_id: "fling".to_string(),
        display_name: "FLiNG".to_string(),
        base_url: "https://flingtrainer.com/".to_string(),
        source_type: "wordpress_rss".to_string(),
        enabled: true,
    }
}

/// Returns the default external trainer sources list (FLiNG only).
pub fn default_external_trainer_sources() -> Vec<ExternalTrainerSourceSubscription> {
    vec![fling_default_source()]
}
