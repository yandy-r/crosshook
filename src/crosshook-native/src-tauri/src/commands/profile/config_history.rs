use crosshook_core::metadata::sha256_hex;
use crosshook_core::metadata::{
    ConfigRevisionSource, MetadataStore, MetadataStoreError, SyncSource, MAX_HISTORY_LIST_LIMIT,
};
use crosshook_core::profile::{
    cap_diff_output_bytes, compute_semantic_diff, compute_unified_diff, GameProfile, ProfileStore,
    SemanticChange, SemanticChangeKind,
};
use crosshook_core::settings::{config_history_max_revisions_from_settings, SettingsStore};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use super::shared::{capture_config_revision, emit_profiles_changed, map_error};

// ── Config history response types ─────────────────────────────────────────────

/// Lightweight summary of a single config revision for list responses.
/// The `snapshot_toml` field is omitted to keep the payload small.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRevisionSummary {
    pub id: i64,
    pub profile_name_at_write: String,
    pub source: String,
    pub content_hash: String,
    pub source_revision_id: Option<i64>,
    pub is_last_known_working: bool,
    pub created_at: String,
}

/// Diff mode requested by the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDiffMode {
    Unified,
    Semantic,
}

impl ConfigDiffMode {
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).filter(|s| !s.is_empty()) {
            Some("semantic") => Self::Semantic,
            _ => Self::Unified,
        }
    }
}

/// A single semantic field change in a TOML snapshot diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSemanticChange {
    pub path: String,
    pub change_type: SemanticChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,
}

impl From<SemanticChange> for ConfigSemanticChange {
    fn from(value: SemanticChange) -> Self {
        Self {
            path: value.path,
            change_type: value.change_type,
            old_value: value.old_value,
            new_value: value.new_value,
        }
    }
}

/// Result from a profile config diff operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiffResult {
    /// The left-side revision id (the selected revision used as the diff base).
    pub revision_id: i64,
    pub revision_source: String,
    pub revision_created_at: String,
    /// Unified diff text in standard format. Empty when the two sides are identical.
    pub diff_text: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    /// True when either input exceeded the line limit and the diff may be incomplete.
    pub truncated: bool,
    /// Mode used to produce this response.
    pub mode: ConfigDiffMode,
    /// Semantic field changes when `mode` is semantic; `None` for unified-only responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_changes: Option<Vec<ConfigSemanticChange>>,
    /// True when semantic parsing failed and the response fell back to unified diff.
    #[serde(default)]
    pub semantic_parse_failed: bool,
}

/// Result from a profile config rollback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRollbackResult {
    /// The revision id that was restored.
    pub restored_revision_id: i64,
    /// The new revision id appended for the rollback event (None when capture was deduped or failed).
    pub new_revision_id: Option<i64>,
    pub profile: GameProfile,
}

// ── Config history Tauri commands ─────────────────────────────────────────────

/// List config revision history for a profile (newest first).
/// Returns an error when the metadata store is unavailable.
/// Returns an empty list when the profile has no recorded revisions yet.
#[tauri::command]
pub fn profile_config_history(
    name: String,
    limit: Option<usize>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<ConfigRevisionSummary>, String> {
    if !metadata_store.is_available() {
        return Err("config history is unavailable — metadata store is not accessible".to_string());
    }

    let profile_id = match metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
    {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let capped_limit = Some(
        limit
            .unwrap_or(MAX_HISTORY_LIST_LIMIT)
            .min(MAX_HISTORY_LIST_LIMIT),
    );
    let rows = metadata_store
        .list_config_revisions(&profile_id, capped_limit)
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| ConfigRevisionSummary {
            id: row.id,
            profile_name_at_write: row.profile_name_at_write,
            source: row.source,
            content_hash: row.content_hash,
            source_revision_id: row.source_revision_id,
            is_last_known_working: row.is_last_known_working,
            created_at: row.created_at,
        })
        .collect())
}

/// Diff a specific revision against the current live profile (when `right_revision_id` is
/// `None`) or against another revision. The left side is `revision_id`; the right side is
/// `right_revision_id` or the current persisted profile. Returns a unified diff string and
/// line-change counts.
#[tauri::command]
pub fn profile_config_diff(
    name: String,
    revision_id: i64,
    right_revision_id: Option<i64>,
    mode: Option<String>,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ConfigDiffResult, String> {
    if !metadata_store.is_available() {
        return Err("config diff is unavailable — metadata store is not accessible".to_string());
    }

    if revision_id <= 0 {
        return Err(format!(
            "revision_id must be a positive integer, got {revision_id}"
        ));
    }
    if let Some(right_id) = right_revision_id {
        if right_id <= 0 {
            return Err(format!(
                "right_revision_id must be a positive integer, got {right_id}"
            ));
        }
    }

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("profile '{name}' has no revision history"))?;

    let left_row = metadata_store
        .get_config_revision(&profile_id, revision_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("revision {revision_id} not found or does not belong to profile '{name}'")
        })?;

    let (right_text, right_label) = if let Some(right_id) = right_revision_id {
        let right_row = metadata_store
            .get_config_revision(&profile_id, right_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!("revision {right_id} not found or does not belong to profile '{name}'")
            })?;
        (right_row.snapshot_toml, format!("revision/{right_id}"))
    } else {
        let current_profile = store.load(&name).map_err(map_error)?;
        let current_toml = toml::to_string_pretty(&current_profile)
            .map_err(|e| format!("failed to serialize current profile: {e}"))?;
        (current_toml, "current".to_string())
    };

    let left_label = format!("revision/{revision_id}");
    let diff_mode = ConfigDiffMode::parse(mode.as_deref());

    let unified = compute_unified_diff(
        &left_label,
        &right_label,
        &left_row.snapshot_toml,
        &right_text,
    );
    let (diff_text, truncated) = cap_diff_output_bytes(unified.diff_text, unified.truncated);

    let (semantic_changes, semantic_parse_failed, semantic_truncated) =
        if diff_mode == ConfigDiffMode::Semantic {
            let semantic = compute_semantic_diff(&left_row.snapshot_toml, &right_text);
            if semantic.parse_failed {
                (None, true, false)
            } else {
                (
                    Some(
                        semantic
                            .changes
                            .into_iter()
                            .map(ConfigSemanticChange::from)
                            .collect(),
                    ),
                    false,
                    semantic.truncated,
                )
            }
        } else {
            (None, false, false)
        };

    Ok(ConfigDiffResult {
        revision_id,
        revision_source: left_row.source,
        revision_created_at: left_row.created_at,
        diff_text,
        added_lines: unified.added_lines,
        removed_lines: unified.removed_lines,
        truncated: truncated || semantic_truncated,
        mode: if semantic_parse_failed {
            ConfigDiffMode::Unified
        } else {
            diff_mode
        },
        semantic_changes,
        semantic_parse_failed,
    })
}

/// Restore a profile from a specific config revision.
/// Verifies that the revision belongs to the named profile, writes the restored TOML
/// via `ProfileStore`, appends a `RollbackApply` revision row with lineage, and emits
/// `profiles-changed`.
#[tauri::command]
pub fn profile_config_rollback(
    name: String,
    revision_id: i64,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<ConfigRollbackResult, String> {
    if !metadata_store.is_available() {
        return Err("rollback is unavailable — metadata store is not accessible".to_string());
    }

    if revision_id <= 0 {
        return Err(format!(
            "revision_id must be a positive integer, got {revision_id}"
        ));
    }

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("profile '{name}' has no revision history"))?;

    let revision = metadata_store
        .get_config_revision(&profile_id, revision_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("revision {revision_id} not found or does not belong to profile '{name}'")
        })?;

    // Integrity check: re-compute the SHA-256 of the stored snapshot and compare
    // against the recorded content_hash to detect DB corruption or tampering.
    {
        let computed = sha256_hex(revision.snapshot_toml.as_bytes());
        if computed != revision.content_hash {
            return Err(format!(
                "integrity check failed for revision {revision_id}: content hash mismatch"
            ));
        }
    }

    let restored_profile: GameProfile = toml::from_str(&revision.snapshot_toml)
        .map_err(|e| format!("failed to parse snapshot for revision {revision_id}: {e}"))?;

    store.save(&name, &restored_profile).map_err(map_error)?;

    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &restored_profile,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(
            %e,
            profile_name = %name,
            revision_id,
            "metadata sync after config rollback failed"
        );
    }

    let max_revisions = settings_store
        .load()
        .map(|s| config_history_max_revisions_from_settings(&s))
        .unwrap_or(crosshook_core::metadata::MAX_CONFIG_REVISIONS_PER_PROFILE);

    let new_revision_id = capture_config_revision(
        &name,
        &restored_profile,
        ConfigRevisionSource::RollbackApply,
        Some(revision_id),
        &metadata_store,
        max_revisions,
    );

    emit_profiles_changed(&app, "rollback");

    Ok(ConfigRollbackResult {
        restored_revision_id: revision_id,
        new_revision_id,
        profile: restored_profile,
    })
}

/// Manually mark a specific revision as the last known-good baseline for a profile.
/// Clears the known-good marker from all other revisions for that profile.
#[tauri::command]
pub fn profile_mark_known_good(
    name: String,
    revision_id: i64,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    if !metadata_store.is_available() {
        return Err(
            "marking known-good is unavailable — metadata store is not accessible".to_string(),
        );
    }

    if revision_id <= 0 {
        return Err(format!(
            "revision_id must be a positive integer, got {revision_id}"
        ));
    }

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("profile '{name}' has no revision history"))?;

    metadata_store
        .set_known_good_revision(&profile_id, revision_id)
        .map_err(|e| match e {
            MetadataStoreError::Corrupt(_) => {
                format!("revision {revision_id} not found or does not belong to profile '{name}'")
            }
            _ => e.to_string(),
        })?;

    Ok(())
}
