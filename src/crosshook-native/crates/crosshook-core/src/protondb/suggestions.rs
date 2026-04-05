use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::launch::catalog::OptimizationEntry;
use crate::profile::GameProfile;

use super::aggregation::{
    is_safe_env_key, is_safe_env_value, BLOCKED_ENV_KEY_PREFIXES, RESERVED_ENV_KEYS,
};
use super::models::{ProtonDbLookupResult, ProtonDbTier};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    New,
    AlreadyApplied,
    Conflict,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSuggestionItem {
    pub catalog_entry_id: String,
    pub label: String,
    pub description: String,
    pub env_pairs: Vec<[String; 2]>,
    pub status: SuggestionStatus,
    pub supporting_report_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarSuggestionItem {
    pub key: String,
    pub value: String,
    pub status: SuggestionStatus,
    pub supporting_report_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptionSuggestionItem {
    pub raw_text: String,
    pub supporting_report_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtonDbSuggestionSet {
    pub catalog_suggestions: Vec<CatalogSuggestionItem>,
    pub env_var_suggestions: Vec<EnvVarSuggestionItem>,
    pub launch_option_suggestions: Vec<LaunchOptionSuggestionItem>,
    pub tier: ProtonDbTier,
    pub total_reports: u32,
    pub is_stale: bool,
}

impl Default for ProtonDbSuggestionSet {
    fn default() -> Self {
        Self {
            catalog_suggestions: Vec::new(),
            env_var_suggestions: Vec::new(),
            launch_option_suggestions: Vec::new(),
            tier: ProtonDbTier::Unknown,
            total_reports: 0,
            is_stale: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AcceptSuggestionRequest {
    #[serde(rename_all = "camelCase")]
    Catalog {
        profile_name: String,
        catalog_entry_id: String,
    },
    #[serde(rename_all = "camelCase")]
    EnvVar {
        profile_name: String,
        env_key: String,
        env_value: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptSuggestionResult {
    pub updated_profile: GameProfile,
    pub applied_keys: Vec<String>,
    pub toggled_option_ids: Vec<String>,
}

/// Validates a proposed env var key and value for safety at write time.
///
/// Returns `true` only when the key and value pass all safety checks:
/// - key conforms to the safe env key format (`is_safe_env_key`)
/// - value conforms to the safe env value format (`is_safe_env_value`)
/// - key is not in the reserved keys list
/// - key does not start with a blocked prefix
///
/// Call this at write time in the accept command — do NOT trust cached
/// suggestion data without re-validating.
pub fn validate_env_suggestion(key: &str, value: &str) -> bool {
    is_safe_env_key(key)
        && is_safe_env_value(value)
        && !RESERVED_ENV_KEYS.contains(&key)
        && !BLOCKED_ENV_KEY_PREFIXES.iter().any(|p| key.starts_with(p))
}

/// Maps (env_key, env_value) -> catalog_entry_id for catalog-matching.
/// Match on full key+value pair, NOT key alone.
fn build_catalog_env_index(catalog: &[OptimizationEntry]) -> HashMap<(String, String), String> {
    let mut index = HashMap::new();
    for entry in catalog {
        for pair in &entry.env {
            index.insert((pair[0].clone(), pair[1].clone()), entry.id.clone());
        }
    }
    index
}

/// Find a catalog entry by ID.
fn find_catalog_entry<'a>(
    catalog: &'a [OptimizationEntry],
    id: &str,
) -> Option<&'a OptimizationEntry> {
    catalog.iter().find(|e| e.id == id)
}

pub fn derive_suggestions(
    lookup: &ProtonDbLookupResult,
    profile: &GameProfile,
    catalog: &[OptimizationEntry],
    dismissed_keys: &HashSet<String>,
) -> ProtonDbSuggestionSet {
    let snapshot = match &lookup.snapshot {
        Some(s) => s,
        None => return ProtonDbSuggestionSet::default(),
    };

    let catalog_env_index = build_catalog_env_index(catalog);
    let enabled_option_ids = &profile.launch.optimizations.enabled_option_ids;
    let custom_env_vars = &profile.launch.custom_env_vars;

    let mut catalog_suggestions: Vec<CatalogSuggestionItem> = Vec::new();
    let mut env_var_suggestions: Vec<EnvVarSuggestionItem> = Vec::new();
    let mut launch_option_suggestions: Vec<LaunchOptionSuggestionItem> = Vec::new();
    let mut seen_catalog_entry_ids: HashSet<String> = HashSet::new();

    for group in &snapshot.recommendation_groups {
        for env_var in &group.env_vars {
            let key = &env_var.key;
            let value = &env_var.value;
            let report_count = env_var.supporting_report_count.unwrap_or(1);

            if let Some(catalog_entry_id) =
                catalog_env_index.get(&(key.clone(), value.clone()))
            {
                // Already seen this catalog entry — skip duplicates, keep highest count (first).
                if seen_catalog_entry_ids.contains(catalog_entry_id) {
                    continue;
                }
                seen_catalog_entry_ids.insert(catalog_entry_id.clone());

                let entry = match find_catalog_entry(catalog, catalog_entry_id) {
                    Some(e) => e,
                    None => continue,
                };

                let status = if dismissed_keys.contains(key) {
                    SuggestionStatus::Dismissed
                } else if enabled_option_ids.contains(catalog_entry_id) {
                    SuggestionStatus::AlreadyApplied
                } else {
                    SuggestionStatus::New
                };

                catalog_suggestions.push(CatalogSuggestionItem {
                    catalog_entry_id: catalog_entry_id.clone(),
                    label: entry.label.clone(),
                    description: entry.description.clone(),
                    env_pairs: entry.env.clone(),
                    status,
                    supporting_report_count: report_count,
                });
            } else {
                let status = if dismissed_keys.contains(key) {
                    SuggestionStatus::Dismissed
                } else if let Some(existing_value) = custom_env_vars.get(key) {
                    if existing_value == value {
                        SuggestionStatus::AlreadyApplied
                    } else {
                        SuggestionStatus::Conflict
                    }
                } else {
                    SuggestionStatus::New
                };

                env_var_suggestions.push(EnvVarSuggestionItem {
                    key: key.clone(),
                    value: value.clone(),
                    status,
                    supporting_report_count: report_count,
                });
            }
        }

        for launch_option in &group.launch_options {
            launch_option_suggestions.push(LaunchOptionSuggestionItem {
                raw_text: launch_option.text.clone(),
                supporting_report_count: launch_option.supporting_report_count.unwrap_or(1),
            });
        }
    }

    catalog_suggestions
        .sort_by(|a, b| b.supporting_report_count.cmp(&a.supporting_report_count));
    env_var_suggestions
        .sort_by(|a, b| b.supporting_report_count.cmp(&a.supporting_report_count));
    launch_option_suggestions
        .sort_by(|a, b| b.supporting_report_count.cmp(&a.supporting_report_count));

    let is_stale = lookup.cache.as_ref().map_or(false, |c| c.is_stale);
    let tier = snapshot.tier.clone();
    let total_reports = snapshot.total_reports.unwrap_or(0);

    ProtonDbSuggestionSet {
        catalog_suggestions,
        env_var_suggestions,
        launch_option_suggestions,
        tier,
        total_reports,
        is_stale,
    }
}
