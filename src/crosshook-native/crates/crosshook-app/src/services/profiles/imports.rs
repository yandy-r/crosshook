//! `record_imports`: post-persistence effects for Lutris imports (D10).
//!
//! Owned by YAN-811 Task 1.5. The core Lutris importer already wrote the
//! TOML; this facade only runs the shared effects (observe `LutrisImport`,
//! revision `Import`, health invalidation) under the writer lock and
//! publishes one `Imported` event naming every recorded profile.

use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};

use super::types::{Effect, ImportedProfileWrite, Persisted, ProfileCommit, ProfileMutation};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, ProfileChangeReason};

impl ProfileWriter {
    /// Records profiles an importer already persisted. An empty list is a
    /// no-op: no effects, no event.
    ///
    /// # Errors
    /// Only pipeline failures (cancellation); advisory metadata failures are
    /// returned as commit warnings.
    pub async fn record_imports(
        &self,
        imports: Vec<ImportedProfileWrite>,
    ) -> Result<ProfileCommit<()>, AppError> {
        self.commit(RecordImports(imports), ProfileChangeReason::Imported)
            .await
    }
}

/// Commit mutation for [`ProfileWriter::record_imports`].
struct RecordImports(Vec<ImportedProfileWrite>);

impl ProfileMutation for RecordImports {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Imported)
    }

    fn persist(self, _ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        let mut effects = Vec::with_capacity(self.0.len() * 3);
        let mut names = Vec::with_capacity(self.0.len());
        for import in self.0 {
            // `Effect::ObserveWrite` derives `<profiles>/<name>.toml`, which is
            // where the importer wrote; `path` is kept for callers' diagnostics.
            effects.push(Effect::ObserveWrite {
                name: import.name.clone(),
                profile: import.profile.clone(),
                source: SyncSource::LutrisImport,
                source_profile_id: None,
            });
            effects.push(Effect::Revision {
                name: import.name.clone(),
                profile: import.profile,
                source: ConfigRevisionSource::Import,
                source_revision_id: None,
            });
            effects.push(Effect::InvalidateHealth {
                name: import.name.clone(),
            });
            names.push(import.name);
        }
        Ok(Persisted {
            output: (),
            effects,
            names: (!names.is_empty()).then_some(names),
        })
    }
}
