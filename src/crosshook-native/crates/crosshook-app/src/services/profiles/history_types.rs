//! Config history, diff and rollback response DTOs (serde shapes unchanged).

use crosshook_core::profile::{GameProfile, SemanticChange, SemanticChangeKind};
use serde::{Deserialize, Serialize};

/// Lightweight summary of a single config revision for list responses.
/// The `snapshot_toml` field is omitted to keep the payload small.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRevisionSummary {
    /// Revision row id.
    pub id: i64,
    /// Profile name when the revision was written.
    pub profile_name_at_write: String,
    /// Write path that produced the revision (`ConfigRevisionSource::as_str`).
    pub source: String,
    /// SHA-256 of the snapshot TOML.
    pub content_hash: String,
    /// Revision this one was restored from, for rollbacks.
    pub source_revision_id: Option<i64>,
    /// Whether this revision is the known-good baseline.
    pub is_last_known_working: bool,
    /// RFC 3339 creation time.
    pub created_at: String,
}

/// Diff mode requested by the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDiffMode {
    /// Unified line diff.
    Unified,
    /// Semantic field-level diff.
    Semantic,
}

/// A single semantic field change in a TOML snapshot diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSemanticChange {
    /// Dotted TOML path of the changed field.
    pub path: String,
    /// Kind of change.
    pub change_type: SemanticChangeKind,
    /// Previous value, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    /// New value, when present.
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
    /// Source of the left-side revision.
    pub revision_source: String,
    /// Creation time of the left-side revision.
    pub revision_created_at: String,
    /// Unified diff text in standard format. Empty when the two sides are identical.
    pub diff_text: String,
    /// Number of added lines.
    pub added_lines: usize,
    /// Number of removed lines.
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
    /// The restored profile.
    pub profile: GameProfile,
}
