use std::borrow::Cow;
use std::fmt;

use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::profile::CompatibilityRating;

pub const PROTONDB_CACHE_NAMESPACE: &str = "protondb";

pub fn normalize_app_id(app_id: &str) -> Option<String> {
    let trimmed = app_id.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn cache_key_for_app_id(app_id: &str) -> String {
    format!("{PROTONDB_CACHE_NAMESPACE}:{}", app_id.trim())
}

/// Exact ProtonDB tier labels preserved as remote strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProtonDbTier {
    Platinum,
    Gold,
    Silver,
    Bronze,
    Borked,
    Native,
    Unknown,
    /// Any future or undocumented tier value should round-trip unchanged.
    Other(String),
}

impl Default for ProtonDbTier {
    fn default() -> Self {
        Self::Unknown
    }
}

impl ProtonDbTier {
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Self::Platinum => Cow::Borrowed("platinum"),
            Self::Gold => Cow::Borrowed("gold"),
            Self::Silver => Cow::Borrowed("silver"),
            Self::Bronze => Cow::Borrowed("bronze"),
            Self::Borked => Cow::Borrowed("borked"),
            Self::Native => Cow::Borrowed("native"),
            Self::Unknown => Cow::Borrowed("unknown"),
            Self::Other(value) => Cow::Borrowed(value.as_str()),
        }
    }

    /// Lossy helper for older CrossHook compatibility surfaces.
    pub fn legacy_compatibility_rating(&self) -> CompatibilityRating {
        match self {
            Self::Platinum | Self::Native => CompatibilityRating::Platinum,
            Self::Gold => CompatibilityRating::Working,
            Self::Silver | Self::Bronze => CompatibilityRating::Partial,
            Self::Borked => CompatibilityRating::Broken,
            Self::Unknown | Self::Other(_) => CompatibilityRating::Unknown,
        }
    }
}

impl Serialize for ProtonDbTier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str().as_ref())
    }
}

impl<'de> Deserialize<'de> for ProtonDbTier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ProtonDbTierVisitor;

        impl<'de> Visitor<'de> for ProtonDbTierVisitor {
            type Value = ProtonDbTier;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a ProtonDB tier string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(match value.trim().to_ascii_lowercase().as_str() {
                    "platinum" => ProtonDbTier::Platinum,
                    "gold" => ProtonDbTier::Gold,
                    "silver" => ProtonDbTier::Silver,
                    "bronze" => ProtonDbTier::Bronze,
                    "borked" => ProtonDbTier::Borked,
                    "native" => ProtonDbTier::Native,
                    "unknown" => ProtonDbTier::Unknown,
                    _ => ProtonDbTier::Other(value.trim().to_string()),
                })
            }
        }

        deserializer.deserialize_str(ProtonDbTierVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProtonDbLookupState {
    #[default]
    Idle,
    Loading,
    Ready,
    Stale,
    Unavailable,
}

/// Advisory record kind used to distinguish plain notes from launch-option text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProtonDbAdvisoryKind {
    #[default]
    Note,
    LaunchOption,
}

/// Freshness metadata for cached ProtonDB lookup data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonDbCacheState {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cache_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fetched_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub from_cache: bool,
    #[serde(default)]
    pub is_stale: bool,
    #[serde(default)]
    pub is_offline: bool,
}

/// Untrusted ProtonDB note text. Render plain text only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonDbAdvisoryNote {
    #[serde(default)]
    pub kind: ProtonDbAdvisoryKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

/// Untrusted ProtonDB launch-option text. Copy-only unless later normalized safely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonDbLaunchOptionSuggestion {
    #[serde(default)]
    pub kind: ProtonDbAdvisoryKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supporting_report_count: Option<u32>,
}

/// Backend-normalized env-var recommendation derived from untrusted ProtonDB launch options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonDbEnvVarSuggestion {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supporting_report_count: Option<u32>,
}

/// Normalized group of advisory guidance returned by ProtonDB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonDbRecommendationGroup {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<ProtonDbAdvisoryNote>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_vars: Vec<ProtonDbEnvVarSuggestion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub launch_options: Vec<ProtonDbLaunchOptionSuggestion>,
}

/// Normalized summary snapshot for a Steam App ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProtonDbSnapshot {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default)]
    pub tier: ProtonDbTier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_reported_tier: Option<ProtonDbTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trending_tier: Option<ProtonDbTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_reports: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommendation_groups: Vec<ProtonDbRecommendationGroup>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fetched_at: String,
}

/// Top-level lookup result used by later cache, IPC, and UI layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProtonDbLookupResult {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default)]
    pub state: ProtonDbLookupState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<ProtonDbCacheState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ProtonDbSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_tier_round_trips_and_preserves_unknown_strings() {
        let known: ProtonDbTier =
            serde_json::from_str(r#""gold""#).expect("gold tier should deserialize");
        assert_eq!(known, ProtonDbTier::Gold);
        assert_eq!(known.legacy_compatibility_rating(), CompatibilityRating::Working);

        let unknown: ProtonDbTier =
            serde_json::from_str(r#""experimental-tier""#).expect("unknown tier should deserialize");
        assert_eq!(unknown, ProtonDbTier::Other("experimental-tier".to_string()));
        assert_eq!(unknown.as_str(), "experimental-tier");
        assert_eq!(serde_json::to_string(&unknown).expect("serialize unknown tier"), r#""experimental-tier""#);
    }

    #[test]
    fn native_and_borked_map_to_lossy_compatibility_scale() {
        assert_eq!(
            ProtonDbTier::Native.legacy_compatibility_rating(),
            CompatibilityRating::Platinum
        );
        assert_eq!(
            ProtonDbTier::Borked.legacy_compatibility_rating(),
            CompatibilityRating::Broken
        );
    }

    #[test]
    fn cache_key_is_namespaced_and_app_id_is_trimmed() {
        assert_eq!(normalize_app_id(" 1245620 "), Some("1245620".to_string()));
        assert_eq!(normalize_app_id("   "), None);
        assert_eq!(cache_key_for_app_id(" 1245620 "), "protondb:1245620");
    }
}
