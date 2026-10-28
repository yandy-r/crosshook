//! Proton migration applies delegated to the shared profile writer.

use crosshook_core::profile::migration::{
    ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult, MigrationApplyResult,
};

use super::ProtonMigrationService;
use crate::services::profiles::ProfileCommit;
use crate::AppError;

impl ProtonMigrationService {
    /// Applies one Proton path migration to the named profile.
    ///
    /// Delegates to `ProfileWriter::apply_proton_migration`: TOCTOU re-check of
    /// the replacement path, write without the MangoHud companion, fail-soft
    /// metadata sync and health invalidation, display-sanitized result paths
    /// (A-4).
    ///
    /// # Errors
    /// The writer's error: `Validation` for an invalid profile name or a
    /// replacement path that no longer exists, `Io` when it cannot be checked.
    pub async fn apply(
        &self,
        request: ApplyMigrationRequest,
    ) -> Result<MigrationApplyResult, AppError> {
        self.writer
            .apply_proton_migration(request)
            .await
            .map(ProfileCommit::into_output)
    }

    /// Applies a batch of Proton path migrations.
    ///
    /// Delegates to `ProfileWriter::apply_batch_migration`: W-4 pre-flight of
    /// every path and profile before any write, per-profile error isolation,
    /// fail-soft metadata sync and health invalidation, display-sanitized
    /// result paths (A-4).
    ///
    /// # Errors
    /// The writer's error for an invalid profile name or a failed pre-flight;
    /// nothing is written in either case.
    pub async fn apply_batch(
        &self,
        request: BatchMigrationRequest,
    ) -> Result<BatchMigrationResult, AppError> {
        self.writer
            .apply_batch_migration(request)
            .await
            .map(ProfileCommit::into_output)
    }
}
