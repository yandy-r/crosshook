//! Proton path migration apply facades (single and batch).
//!
//! Owned by YAN-811 Task 1.5. Every request's profile name is validated and
//! trimmed before the commit. Each item is load →
//! `prepare_profile_migration` → `save_without_companion` (migration never
//! rewrites the MangoHud companion). Only `Applied` items observe
//! `AppMigration`, record a `Migration` config revision, invalidate their
//! cached health snapshot and appear in the single `Migrated` event;
//! `AlreadyValid`/`Failed` items have no effects.

use std::path::Path;

use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};
use crosshook_core::profile::migration::{
    prepare_profile_migration, ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult,
    MigrationApplyResult, MigrationOutcome,
};
use crosshook_core::profile::ProfileStore;

use super::types::{Effect, Persisted, ProfileCommit, ProfileMutation};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::presenter::{display_path_tilde, sanitize_message};
use crate::{AppError, AppErrorKind, ProfileChangeReason};

impl ProfileWriter {
    /// Applies one Proton path migration to the named profile (trimmed).
    ///
    /// The replacement path is re-verified under the writer lock right
    /// before the write (TOCTOU). A load or write failure is reported as a
    /// `Failed` result, not an error. Returned paths are display-sanitized.
    ///
    /// # Errors
    /// `Validation` when the profile name is invalid or the replacement path
    /// no longer exists, `Io` when it cannot be checked; nothing is written
    /// in either case.
    pub async fn apply_proton_migration(
        &self,
        mut request: ApplyMigrationRequest,
    ) -> Result<ProfileCommit<MigrationApplyResult>, AppError> {
        request.profile_name = ProfileStore::validate_profile_name(&request.profile_name)?;
        self.commit(SingleMigration(request), ProfileChangeReason::Migrated)
            .await
    }

    /// Applies a batch of Proton path migrations.
    ///
    /// Every profile name is validated and trimmed first. Pre-flight (W-4)
    /// then verifies every replacement path and loads every profile before
    /// any write; any failure aborts with zero writes and no event.
    /// Afterwards each item is applied independently, so one failure does not
    /// abort the rest. Returned paths are display-sanitized.
    ///
    /// # Errors
    /// The first invalid profile name or pre-flight failure; nothing is
    /// written.
    pub async fn apply_batch_migration(
        &self,
        mut request: BatchMigrationRequest,
    ) -> Result<ProfileCommit<BatchMigrationResult>, AppError> {
        for item in &mut request.migrations {
            item.profile_name = ProfileStore::validate_profile_name(&item.profile_name)?;
        }
        self.commit(BatchMigration(request), ProfileChangeReason::Migrated)
            .await
    }
}

/// Commit mutation for [`ProfileWriter::apply_proton_migration`].
struct SingleMigration(ApplyMigrationRequest);

impl ProfileMutation for SingleMigration {
    type Output = MigrationApplyResult;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Migrated)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<Self::Output>, AppError> {
        let request = self.0;
        // TOCTOU mitigation: re-verify replacement path immediately before write.
        match Path::new(&request.new_path).try_exists() {
            Ok(true) => {}
            Ok(false) => {
                return Err(AppError::validation(
                    "new_path",
                    format!("Replacement path no longer exists: {}", request.new_path),
                ));
            }
            Err(err) => {
                return Err(AppError::new(
                    AppErrorKind::Io,
                    format!(
                        "Could not verify replacement path {}: {err}",
                        request.new_path
                    ),
                ));
            }
        }

        let mut effects = Vec::new();
        let mut names = Vec::new();
        let output = apply_one(ctx.profiles, &request, &mut effects, &mut names);
        Ok(Persisted {
            output,
            effects,
            names: (!names.is_empty()).then_some(names),
        })
    }
}

/// Commit mutation for [`ProfileWriter::apply_batch_migration`].
struct BatchMigration(BatchMigrationRequest);

impl ProfileMutation for BatchMigration {
    type Output = BatchMigrationResult;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Migrated)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<Self::Output>, AppError> {
        let migrations = self.0.migrations;
        preflight(ctx.profiles, &migrations)?;

        let mut effects = Vec::new();
        let mut names = Vec::new();
        let mut results = Vec::with_capacity(migrations.len());
        let (mut applied_count, mut failed_count, mut skipped_count) = (0usize, 0usize, 0usize);
        for request in &migrations {
            let result = apply_one(ctx.profiles, request, &mut effects, &mut names);
            match result.outcome {
                MigrationOutcome::Applied => applied_count += 1,
                MigrationOutcome::AlreadyValid => skipped_count += 1,
                MigrationOutcome::Failed => failed_count += 1,
            }
            results.push(result);
        }

        Ok(Persisted {
            output: BatchMigrationResult {
                results,
                applied_count,
                failed_count,
                skipped_count,
            },
            effects,
            names: (!names.is_empty()).then_some(names),
        })
    }
}

/// W-4: verifies every replacement path and loads every profile before any
/// write.
fn preflight(store: &ProfileStore, migrations: &[ApplyMigrationRequest]) -> Result<(), AppError> {
    for request in migrations {
        match Path::new(&request.new_path).try_exists() {
            Ok(true) => {}
            Ok(false) => {
                return Err(AppError::validation(
                    "new_path",
                    format!(
                        "Pre-flight failed for '{}': replacement path does not exist: {}",
                        request.profile_name, request.new_path
                    ),
                ));
            }
            Err(err) => {
                return Err(AppError::new(
                    AppErrorKind::Io,
                    format!(
                        "Pre-flight failed for '{}': could not verify replacement path: {err}",
                        request.profile_name
                    ),
                ));
            }
        }

        if let Err(err) = store.load(&request.profile_name) {
            // Today's message, sanitized by `AppError::new`; kind from the store error.
            let message = format!(
                "Pre-flight failed for '{}': could not load profile: {err}",
                request.profile_name
            );
            return Err(AppError::new(AppError::from(err).kind(), message));
        }
    }
    Ok(())
}

/// Applies one migration (load → prepare → `save_without_companion`) and,
/// only when it was applied, queues its effects (observe → `Migration`
/// revision → health invalidation) and records its name once.
/// The returned result carries display-sanitized paths and error.
fn apply_one(
    store: &ProfileStore,
    request: &ApplyMigrationRequest,
    effects: &mut Vec<Effect>,
    names: &mut Vec<String>,
) -> MigrationApplyResult {
    let mut result = match store.load(&request.profile_name) {
        Ok(mut profile) => {
            let mut result = prepare_profile_migration(&mut profile, request);
            if result.outcome == MigrationOutcome::Applied {
                match store.save_without_companion(&request.profile_name, &profile) {
                    Ok(()) => {
                        effects.push(Effect::ObserveWrite {
                            name: request.profile_name.clone(),
                            profile: profile.clone(),
                            source: SyncSource::AppMigration,
                            source_profile_id: None,
                        });
                        effects.push(Effect::Revision {
                            name: request.profile_name.clone(),
                            profile,
                            source: ConfigRevisionSource::Migration,
                            source_revision_id: None,
                        });
                        effects.push(Effect::InvalidateHealth {
                            name: request.profile_name.clone(),
                        });
                        if !names.contains(&request.profile_name) {
                            names.push(request.profile_name.clone());
                        }
                    }
                    Err(err) => {
                        result.outcome = MigrationOutcome::Failed;
                        result.error = Some(err.to_string());
                    }
                }
            }
            result
        }
        Err(err) => MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path: String::new(),
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(err.to_string()),
        },
    };

    // A-4: sanitize IPC-bound path strings (and the error, which may embed them).
    result.old_path = display_path_tilde(&result.old_path);
    result.new_path = display_path_tilde(&result.new_path);
    result.error = result.error.as_deref().map(sanitize_message);
    result
}
