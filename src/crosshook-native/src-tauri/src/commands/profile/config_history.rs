use crosshook_core::metadata::sha256_hex;
use crosshook_core::metadata::{
    ConfigRevisionSource, MetadataStore, MetadataStoreError, SyncSource, MAX_HISTORY_LIST_LIMIT,
};
use crosshook_core::profile::{GameProfile, ProfileStore};
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

// ── Unified diff helper ───────────────────────────────────────────────────────

const DIFF_CONTEXT_LINES: usize = 3;
/// Maximum lines per side considered for the diff. Profiles are small in practice
/// but this caps the O(m*n) LCS table at a safe memory bound.
const DIFF_MAX_LINES: usize = 2000;
/// Maximum byte length of a computed diff output returned to the frontend.
/// Prevents large IPC payloads when both sides have many long changed lines.
const MAX_DIFF_OUTPUT_BYTES: usize = 512 * 1024;

/// Compute a unified diff between two text strings using an LCS-based algorithm.
/// Returns `(diff_text, added_lines, removed_lines, truncated)`. `diff_text` is
/// empty when the two inputs are identical. `truncated` is true when either input
/// exceeds `DIFF_MAX_LINES` and the diff may be incomplete.
fn compute_unified_diff(
    old_label: &str,
    new_label: &str,
    old_text: &str,
    new_text: &str,
) -> (String, usize, usize, bool) {
    if old_text == new_text {
        return (String::new(), 0, 0, false);
    }

    let old_total = old_text.lines().count();
    let new_total = new_text.lines().count();
    let truncated = old_total > DIFF_MAX_LINES || new_total > DIFF_MAX_LINES;

    let old_lines: Vec<&str> = old_text.lines().take(DIFF_MAX_LINES).collect();
    let new_lines: Vec<&str> = new_text.lines().take(DIFF_MAX_LINES).collect();

    let m = old_lines.len();
    let n = new_lines.len();

    // Build LCS table (O(m*n) time and space, bounded by DIFF_MAX_LINES^2)
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if old_lines[i - 1] == new_lines[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack to produce ordered edit ops
    #[derive(Clone, Copy)]
    enum Op {
        Equal(usize, usize),
        Delete(usize),
        Insert(usize),
    }

    let mut ops: Vec<Op> = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push(Op::Equal(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(Op::Insert(j - 1));
            j -= 1;
        } else {
            ops.push(Op::Delete(i - 1));
            i -= 1;
        }
    }
    ops.reverse();

    // Collect indices of changed ops
    let change_positions: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !matches!(op, Op::Equal(_, _)))
        .map(|(idx, _)| idx)
        .collect();

    if change_positions.is_empty() {
        return (String::new(), 0, 0, truncated);
    }

    // Group changes into hunks with context lines
    let mut hunk_ranges: Vec<(usize, usize)> = Vec::new();
    let last_op = ops.len() - 1;
    let mut hstart = change_positions[0].saturating_sub(DIFF_CONTEXT_LINES);
    let mut hend = (change_positions[0] + DIFF_CONTEXT_LINES).min(last_op);

    for &pos in change_positions.iter().skip(1) {
        if pos <= hend + DIFF_CONTEXT_LINES {
            hend = (pos + DIFF_CONTEXT_LINES).min(last_op);
        } else {
            hunk_ranges.push((hstart, hend));
            hstart = pos.saturating_sub(DIFF_CONTEXT_LINES);
            hend = (pos + DIFF_CONTEXT_LINES).min(last_op);
        }
    }
    hunk_ranges.push((hstart, hend));

    let mut output = format!("--- {old_label}\n+++ {new_label}\n");
    let mut added = 0usize;
    let mut removed = 0usize;

    for (hstart, hend) in hunk_ranges {
        let hunk = &ops[hstart..=hend];

        let old_start = hunk
            .iter()
            .find_map(|op| match op {
                Op::Equal(oi, _) | Op::Delete(oi) => Some(oi + 1),
                Op::Insert(_) => None,
            })
            .unwrap_or(0);

        let new_start = hunk
            .iter()
            .find_map(|op| match op {
                Op::Equal(_, ni) | Op::Insert(ni) => Some(ni + 1),
                Op::Delete(_) => None,
            })
            .unwrap_or(0);

        let old_count = hunk
            .iter()
            .filter(|op| !matches!(op, Op::Insert(_)))
            .count();
        let new_count = hunk
            .iter()
            .filter(|op| !matches!(op, Op::Delete(_)))
            .count();

        output.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));

        for op in hunk {
            match op {
                Op::Equal(oi, _) => {
                    output.push(' ');
                    output.push_str(old_lines[*oi]);
                    output.push('\n');
                }
                Op::Delete(oi) => {
                    output.push('-');
                    output.push_str(old_lines[*oi]);
                    output.push('\n');
                    removed += 1;
                }
                Op::Insert(ni) => {
                    output.push('+');
                    output.push_str(new_lines[*ni]);
                    output.push('\n');
                    added += 1;
                }
            }
        }
    }

    (output, added, removed, truncated)
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
    let (diff_text, added_lines, removed_lines, truncated) = compute_unified_diff(
        &left_label,
        &right_label,
        &left_row.snapshot_toml,
        &right_text,
    );

    if diff_text.len() > MAX_DIFF_OUTPUT_BYTES {
        return Err(format!(
            "diff output for revision {revision_id} exceeds the {MAX_DIFF_OUTPUT_BYTES}-byte limit ({} bytes)",
            diff_text.len()
        ));
    }

    Ok(ConfigDiffResult {
        revision_id,
        revision_source: left_row.source,
        revision_created_at: left_row.created_at,
        diff_text,
        added_lines,
        removed_lines,
        truncated,
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

    let new_revision_id = capture_config_revision(
        &name,
        &restored_profile,
        ConfigRevisionSource::RollbackApply,
        Some(revision_id),
        &metadata_store,
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
