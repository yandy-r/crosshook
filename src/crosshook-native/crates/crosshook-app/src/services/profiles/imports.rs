//! Lutris and community import facades.
//!
//! Both run the core importer under the writer lock (D3), so an import
//! cannot interleave with another profile write, then queue the shared
//! effects (observe → `Import` revision → health invalidation) and publish
//! one `Imported` event naming every written profile.

use std::path::PathBuf;

use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};
use crosshook_core::profile::lutris_import::apply_lutris_import;
use crosshook_core::profile::{
    import_community_profile, CommunityImportResult, GameProfile, LutrisImportEntry,
    LutrisImportOutcome, LutrisImportResult,
};

use super::types::{Effect, Persisted, ProfileCommit, ProfileMutation};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, ProfileChangeReason};

impl ProfileWriter {
    /// Imports the selected Lutris entries, each under a unique profile name.
    /// Per-entry failures and skipped entries are reported in the result,
    /// not as errors. Publishes `Imported` with the written names; nothing
    /// when no entry was written.
    ///
    /// # Errors
    /// Only pipeline failures (cancellation); advisory metadata failures are
    /// returned as commit warnings.
    pub async fn import_lutris(
        &self,
        entries: Vec<LutrisImportEntry>,
    ) -> Result<ProfileCommit<LutrisImportResult>, AppError> {
        self.commit(LutrisImport(entries), ProfileChangeReason::Imported)
            .await
    }

    /// Imports the community profile manifest at `path` (overwriting a
    /// profile of the derived name) and seeds its display-only version
    /// snapshot from the manifest metadata. The caller has already checked
    /// that `path` is allowed. Publishes `Imported` with the profile name.
    ///
    /// # Errors
    /// Unreadable or invalid manifest, unsupported schema version, or a
    /// profile write failure; nothing is published in those cases.
    pub async fn import_community(
        &self,
        path: PathBuf,
    ) -> Result<ProfileCommit<CommunityImportResult>, AppError> {
        self.commit(CommunityImport(path), ProfileChangeReason::Imported)
            .await
    }
}

/// Observe (`source`), `Import` revision and health invalidation of one
/// imported profile.
fn import_effects(effects: &mut Vec<Effect>, name: &str, profile: GameProfile, source: SyncSource) {
    effects.push(Effect::ObserveWrite {
        name: name.to_string(),
        profile: profile.clone(),
        source,
        source_profile_id: None,
    });
    effects.push(Effect::Revision {
        name: name.to_string(),
        profile,
        source: ConfigRevisionSource::Import,
        source_revision_id: None,
    });
    effects.push(Effect::InvalidateHealth {
        name: name.to_string(),
    });
}

/// Commit mutation for [`ProfileWriter::import_lutris`].
struct LutrisImport(Vec<LutrisImportEntry>);

impl ProfileMutation for LutrisImport {
    type Output = LutrisImportResult;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Imported)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<LutrisImportResult>, AppError> {
        let result = apply_lutris_import(ctx.profiles, self.0);
        let mut effects = Vec::with_capacity(result.imported_count * 3);
        let mut names = Vec::with_capacity(result.imported_count);
        for entry in &result.results {
            if entry.outcome != LutrisImportOutcome::Imported {
                continue;
            }
            let Some(name) = entry.profile_name.as_deref() else {
                continue;
            };
            import_effects(
                &mut effects,
                name,
                entry.entry.mapped.clone(),
                SyncSource::LutrisImport,
            );
            names.push(name.to_string());
        }
        Ok(Persisted {
            output: result,
            effects,
            names: (!names.is_empty()).then_some(names),
        })
    }
}

/// Commit mutation for [`ProfileWriter::import_community`].
struct CommunityImport(PathBuf);

impl ProfileMutation for CommunityImport {
    type Output = CommunityImportResult;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Imported)
    }

    fn persist(
        self,
        ctx: &CommitContext<'_>,
    ) -> Result<Persisted<CommunityImportResult>, AppError> {
        let result = import_community_profile(&self.0, &ctx.profiles.base_path)?;
        let name = result.profile_name.clone();
        let mut effects = Vec::with_capacity(4);
        import_effects(
            &mut effects,
            &name,
            result.profile.clone(),
            SyncSource::Import,
        );

        // Display-only version seed from community metadata (BR-8/W3):
        // game_version → human_game_ver, steam_build_id stays None.
        let metadata = &result.manifest.metadata;
        let non_empty = |value: &str| (!value.is_empty()).then(|| value.to_string());
        let trainer_version = non_empty(&metadata.trainer_version);
        let human_game_ver = non_empty(&metadata.game_version);
        if trainer_version.is_some() || human_game_ver.is_some() {
            effects.push(Effect::SeedVersionSnapshot {
                name: name.clone(),
                steam_app_id: result.profile.steam.app_id.trim().to_string(),
                trainer_version,
                human_game_ver,
            });
        }

        Ok(Persisted {
            output: result,
            effects,
            names: Some(vec![name]),
        })
    }
}
