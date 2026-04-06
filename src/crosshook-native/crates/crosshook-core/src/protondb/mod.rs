//! ProtonDB lookup contract and normalized advisory models.

mod aggregation;
mod client;
pub mod models;
pub mod suggestions;

pub use client::lookup_protondb;
pub use models::{
    cache_key_for_app_id, normalize_app_id, ProtonDbAdvisoryKind, ProtonDbAdvisoryNote,
    ProtonDbCacheState, ProtonDbEnvVarSuggestion, ProtonDbLaunchOptionSuggestion,
    ProtonDbLookupResult, ProtonDbLookupState, ProtonDbRecommendationGroup, ProtonDbSnapshot,
    ProtonDbTier, PROTONDB_CACHE_NAMESPACE,
};
pub use suggestions::{
    derive_suggestions, validate_env_suggestion, AcceptSuggestionRequest, AcceptSuggestionResult,
    CatalogSuggestionItem, EnvVarSuggestionItem, LaunchOptionSuggestionItem, ProtonDbSuggestionSet,
    SuggestionStatus,
};

#[cfg(test)]
mod tests;
