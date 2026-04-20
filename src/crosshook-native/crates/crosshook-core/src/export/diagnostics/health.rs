use crate::profile::health::batch_check_health;
use crate::profile::ProfileStore;

/// Runs batch health check across all profiles and serializes the result.
pub(super) fn collect_health_summary(store: &ProfileStore) -> String {
    let summary = batch_check_health(store);
    serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string())
}
