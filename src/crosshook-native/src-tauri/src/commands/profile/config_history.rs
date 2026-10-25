use crosshook_app::services::profiles::{
    ConfigDiffResult, ConfigRevisionSummary, ConfigRollbackResult, ProfileCommit,
};
use crosshook_app::ProfileWriter;
use tauri::State;

/// List config revision history for a profile (newest first).
/// Returns an error when the metadata store is unavailable.
/// Returns an empty list when the profile has no recorded revisions yet.
#[tauri::command]
pub async fn profile_config_history(
    name: String,
    limit: Option<usize>,
    writer: State<'_, ProfileWriter>,
) -> Result<Vec<ConfigRevisionSummary>, String> {
    writer
        .config_history(name, limit)
        .await
        .map_err(|e| e.to_string())
}

/// Diff a specific revision against the current live profile (when `right_revision_id` is
/// `None`) or against another revision. The left side is `revision_id`; the right side is
/// `right_revision_id` or the current persisted profile. Returns a unified diff string and
/// line-change counts.
#[tauri::command]
pub async fn profile_config_diff(
    name: String,
    revision_id: i64,
    right_revision_id: Option<i64>,
    mode: Option<String>,
    writer: State<'_, ProfileWriter>,
) -> Result<ConfigDiffResult, String> {
    writer
        .config_diff(name, revision_id, right_revision_id, mode)
        .await
        .map_err(|e| e.to_string())
}

/// Restore a profile from a specific config revision.
/// Verifies ownership and integrity before writing the restored TOML, appends a
/// `RollbackApply` revision row with lineage, and emits `profiles-changed`.
#[tauri::command]
pub async fn profile_config_rollback(
    name: String,
    revision_id: i64,
    writer: State<'_, ProfileWriter>,
) -> Result<ConfigRollbackResult, String> {
    writer
        .config_rollback(name, revision_id)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|e| e.to_string())
}

/// Manually mark a specific revision as the last known-good baseline for a profile.
/// Clears the known-good marker from all other revisions for that profile.
#[tauri::command]
pub async fn profile_mark_known_good(
    name: String,
    revision_id: i64,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .mark_known_good(name, revision_id)
        .await
        .map_err(|e| e.to_string())
}
