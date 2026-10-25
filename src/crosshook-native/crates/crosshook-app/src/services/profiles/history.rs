//! Config history, diff, rollback and known-good facades.
//!
//! History, diff and known-good run directly on the blocking pool without the
//! writer lock: they never touch profile TOML. Rollback is the only content
//! write and goes through the commit pipeline, verifying the stored revision
//! (SHA-256 plus HMAC, ADR-0005) before anything is written. Every operation
//! needs the metadata store and fails with `Unavailable` without it (D6).

use crosshook_core::metadata::{
    ConfigRevisionRow, ConfigRevisionSource, MetadataStore, MetadataStoreError, SyncSource,
    MAX_HISTORY_LIST_LIMIT,
};
use crosshook_core::profile::{
    cap_diff_output_bytes, compute_semantic_diff, compute_unified_diff, GameProfile,
};

use super::history_types::{
    ConfigDiffMode, ConfigDiffResult, ConfigRevisionSummary, ConfigRollbackResult,
    ConfigSemanticChange,
};
use super::run_blocking;
use super::types::{Effect, Persisted, ProfileCommit, ProfileMutation};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, AppErrorKind, ProfileChangeReason};

impl ConfigDiffMode {
    /// Parses the frontend `mode` argument; anything but `semantic` is unified.
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).filter(|s| !s.is_empty()) {
            Some("semantic") => Self::Semantic,
            _ => Self::Unified,
        }
    }
}

impl ProfileWriter {
    /// Lists config revisions of profile `name`, newest first, capped at
    /// [`MAX_HISTORY_LIST_LIMIT`]. A profile without recorded revisions yields
    /// an empty list.
    ///
    /// # Errors
    /// `Unavailable` when the metadata store is disabled; metadata read errors.
    pub async fn config_history(
        &self,
        name: String,
        limit: Option<usize>,
    ) -> Result<Vec<ConfigRevisionSummary>, AppError> {
        require_metadata(&self.metadata, "config history")?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let Some(profile_id) = metadata.lookup_profile_id(&name)? else {
                return Ok(Vec::new());
            };
            let limit = limit
                .unwrap_or(MAX_HISTORY_LIST_LIMIT)
                .min(MAX_HISTORY_LIST_LIMIT);
            let rows = metadata.list_config_revisions(&profile_id, Some(limit))?;
            Ok(rows.into_iter().map(revision_summary).collect())
        })
        .await
    }

    /// Diffs revision `revision_id` (left) against `right_revision_id` or,
    /// when `None`, the current persisted profile. `mode` selects a unified
    /// (default) or semantic diff; a semantic diff whose snapshots do not
    /// parse falls back to unified with `semantic_parse_failed` set.
    ///
    /// # Errors
    /// `Unavailable` without metadata, `Validation` for non-positive ids,
    /// `NotFound` when a revision does not belong to `name` or the profile is
    /// missing, metadata or profile read errors.
    pub async fn config_diff(
        &self,
        name: String,
        revision_id: i64,
        right_revision_id: Option<i64>,
        mode: Option<String>,
    ) -> Result<ConfigDiffResult, AppError> {
        require_metadata(&self.metadata, "config diff")?;
        require_positive("revision_id", revision_id)?;
        if let Some(right_id) = right_revision_id {
            require_positive("right_revision_id", right_id)?;
        }
        let metadata = self.metadata.clone();
        let profiles = self.profiles.clone();
        run_blocking(move || {
            let profile_id = require_profile_id(&metadata, &name)?;
            let left = scoped_revision(&metadata, &profile_id, &name, revision_id)?;
            let (right_text, right_label) = match right_revision_id {
                Some(right_id) => {
                    let right = scoped_revision(&metadata, &profile_id, &name, right_id)?;
                    (right.snapshot_toml, format!("revision/{right_id}"))
                }
                None => {
                    let current = profiles.load(&name)?;
                    let current_toml = toml::to_string_pretty(&current).map_err(|error| {
                        AppError::new(
                            AppErrorKind::Internal,
                            format!("failed to serialize current profile: {error}"),
                        )
                    })?;
                    (current_toml, "current".to_string())
                }
            };
            Ok(build_diff(
                left,
                &right_text,
                &right_label,
                ConfigDiffMode::parse(mode.as_deref()),
            ))
        })
        .await
    }

    /// Restores profile `name` from revision `revision_id`.
    ///
    /// The revision must belong to `name` and pass the integrity check before
    /// anything is written. The verified snapshot is saved wholesale, then a
    /// `RollbackApply` revision with lineage `source_revision_id = revision_id`
    /// is recorded (deduped against the latest revision) and
    /// `ProfileChangeReason::RolledBack` is published.
    ///
    /// # Errors
    /// `Unavailable` without metadata, `Validation` for a non-positive id, an
    /// integrity failure or an unparsable snapshot, `NotFound` when the
    /// revision does not belong to `name`, `Io` when the TOML write fails.
    pub async fn config_rollback(
        &self,
        name: String,
        revision_id: i64,
    ) -> Result<ProfileCommit<ConfigRollbackResult>, AppError> {
        require_metadata(&self.metadata, "rollback")?;
        require_positive("revision_id", revision_id)?;
        let ProfileCommit {
            output: profile,
            new_revision_id,
            warnings,
        } = self
            .commit(
                RollbackRestore { name, revision_id },
                ProfileChangeReason::RolledBack,
            )
            .await?;
        Ok(ProfileCommit {
            output: ConfigRollbackResult {
                restored_revision_id: revision_id,
                new_revision_id,
                profile,
            },
            new_revision_id,
            warnings,
        })
    }

    /// Marks revision `revision_id` as the known-good baseline of profile
    /// `name`, clearing the marker on its other revisions. Metadata only: no
    /// TOML write and no profile event.
    ///
    /// # Errors
    /// `Unavailable` without metadata, `Validation` for a non-positive id,
    /// `NotFound` when the revision does not belong to `name`.
    pub async fn mark_known_good(&self, name: String, revision_id: i64) -> Result<(), AppError> {
        require_metadata(&self.metadata, "marking known-good")?;
        require_positive("revision_id", revision_id)?;
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = require_profile_id(&metadata, &name)?;
            metadata
                .set_known_good_revision(&profile_id, revision_id)
                .map_err(|error| match error {
                    // The store reports a revision outside the profile as `Corrupt`.
                    MetadataStoreError::Corrupt(_) => revision_not_found(revision_id, &name),
                    other => other.into(),
                })
        })
        .await
    }
}

/// Rollback of one profile to a stored, verified revision snapshot.
struct RollbackRestore {
    name: String,
    revision_id: i64,
}

impl ProfileMutation for RollbackRestore {
    type Output = GameProfile;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::RolledBack)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<GameProfile>, AppError> {
        let Self { name, revision_id } = self;
        let profile_id = require_profile_id(ctx.metadata, &name)?;
        let revision = scoped_revision(ctx.metadata, &profile_id, &name, revision_id)?;
        // Integrity first (ADR-0005): SHA-256 always, HMAC when a key is loaded.
        ctx.metadata.verify_config_revision(&revision)?;
        let restored: GameProfile = toml::from_str(&revision.snapshot_toml).map_err(|error| {
            AppError::validation(
                "revision",
                format!("failed to parse snapshot for revision {revision_id}: {error}"),
            )
        })?;
        ctx.profiles.save(&name, &restored)?;
        Ok(Persisted {
            output: restored.clone(),
            effects: vec![
                Effect::ObserveWrite {
                    name: name.clone(),
                    profile: restored.clone(),
                    source: SyncSource::AppWrite,
                    source_profile_id: None,
                },
                Effect::Revision {
                    name: name.clone(),
                    profile: restored,
                    source: ConfigRevisionSource::RollbackApply,
                    source_revision_id: Some(revision_id),
                },
                Effect::InvalidateHealth { name: name.clone() },
            ],
            names: Some(vec![name]),
        })
    }
}

/// Builds the diff response: unified diff (line and byte capped) plus the
/// optional semantic diff, falling back to unified when parsing fails.
fn build_diff(
    left: ConfigRevisionRow,
    right_text: &str,
    right_label: &str,
    mode: ConfigDiffMode,
) -> ConfigDiffResult {
    let left_label = format!("revision/{}", left.id);
    let unified = compute_unified_diff(&left_label, right_label, &left.snapshot_toml, right_text);
    let (diff_text, line_or_byte_truncated) =
        cap_diff_output_bytes(unified.diff_text, unified.truncated);

    let (semantic_changes, semantic_parse_failed, semantic_truncated) =
        if mode == ConfigDiffMode::Semantic {
            let semantic = compute_semantic_diff(&left.snapshot_toml, right_text);
            if semantic.parse_failed {
                (None, true, false)
            } else {
                let changes = semantic
                    .changes
                    .into_iter()
                    .map(ConfigSemanticChange::from)
                    .collect();
                (Some(changes), false, semantic.truncated)
            }
        } else {
            (None, false, false)
        };

    ConfigDiffResult {
        revision_id: left.id,
        revision_source: left.source,
        revision_created_at: left.created_at,
        diff_text,
        added_lines: unified.added_lines,
        removed_lines: unified.removed_lines,
        truncated: line_or_byte_truncated || semantic_truncated,
        mode: if semantic_parse_failed {
            ConfigDiffMode::Unified
        } else {
            mode
        },
        semantic_changes,
        semantic_parse_failed,
    }
}

fn revision_summary(row: ConfigRevisionRow) -> ConfigRevisionSummary {
    ConfigRevisionSummary {
        id: row.id,
        profile_name_at_write: row.profile_name_at_write,
        source: row.source,
        content_hash: row.content_hash,
        source_revision_id: row.source_revision_id,
        is_last_known_working: row.is_last_known_working,
        created_at: row.created_at,
    }
}

fn require_metadata(metadata: &MetadataStore, operation: &str) -> Result<(), AppError> {
    if metadata.is_available() {
        Ok(())
    } else {
        Err(AppError::new(
            AppErrorKind::Unavailable,
            format!("{operation} is unavailable — metadata store is not accessible"),
        ))
    }
}

fn require_positive(field: &'static str, id: i64) -> Result<(), AppError> {
    if id > 0 {
        Ok(())
    } else {
        Err(AppError::validation(
            field,
            format!("{field} must be a positive integer, got {id}"),
        ))
    }
}

fn require_profile_id(metadata: &MetadataStore, name: &str) -> Result<String, AppError> {
    metadata.lookup_profile_id(name)?.ok_or_else(|| {
        AppError::new(
            AppErrorKind::NotFound,
            format!("profile '{name}' has no revision history"),
        )
    })
}

/// Loads revision `revision_id` only when it belongs to `profile_id`.
fn scoped_revision(
    metadata: &MetadataStore,
    profile_id: &str,
    name: &str,
    revision_id: i64,
) -> Result<ConfigRevisionRow, AppError> {
    metadata
        .get_config_revision(profile_id, revision_id)?
        .ok_or_else(|| revision_not_found(revision_id, name))
}

fn revision_not_found(revision_id: i64, name: &str) -> AppError {
    AppError::new(
        AppErrorKind::NotFound,
        format!("revision {revision_id} not found or does not belong to profile '{name}'"),
    )
}
