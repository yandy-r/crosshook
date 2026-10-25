//! Full save, delete, duplicate, rename, legacy import and TOML export facades.
//!
//! Each mutation validates, loads and persists the profile TOML exactly once
//! under the writer lock, then hands its metadata effects (observe → revision
//! → health invalidation) to the commit pipeline in `writer.rs`.

use std::path::PathBuf;

use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};
use crosshook_core::profile::{
    profile_to_shareable_toml_with_options, validate_steam_app_id, DuplicateProfileResult,
    GameProfile, ProfileStore, ProfileStoreError, ShareableTomlOptions,
};

use super::defaults_art::{apply_new_profile_defaults, auto_import_custom_art};
use super::launcher_cleanup::cleanup_launchers_for_profile_delete;
use super::types::{Effect, Persisted, ProfileCommit, ProfileMutation};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, AppErrorKind, ProfileChangeReason};

impl ProfileWriter {
    /// Saves the editor's full profile as `name` (trimmed).
    ///
    /// Existing profiles keep the unknown TOML keys already on disk; new
    /// profiles get the settings creation defaults and the default bundled
    /// optimization preset. Custom art outside the managed media directory is
    /// imported (fail-soft). Publishes `Saved`.
    ///
    /// # Errors
    /// `Validation` for an invalid name (before any side effect) or
    /// `runtime.steam_app_id`; profile load/save or settings read failures.
    /// Metadata failures are commit warnings.
    pub async fn save(
        &self,
        name: String,
        profile: GameProfile,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = ProfileStore::validate_profile_name(&name)?;
        self.commit(SaveProfile { name, profile }, ProfileChangeReason::Saved)
            .await
    }

    /// Deletes `name` (trimmed) after a best-effort cleanup of its exported
    /// launchers. Publishes `Deleted` with the removed name.
    ///
    /// # Errors
    /// Invalid name, missing profile or filesystem failure.
    pub async fn delete(&self, name: String) -> Result<ProfileCommit<()>, AppError> {
        let name = ProfileStore::validate_profile_name(&name)?;
        self.commit(DeleteProfile { name }, ProfileChangeReason::Deleted)
            .await
    }

    /// Copies `name` (trimmed) under a unique `(Copy)` name, recording the
    /// source profile as lineage. The returned profile has unknown keys
    /// cleared; the copy on disk keeps them. Publishes `Duplicated` with the
    /// copy name.
    ///
    /// # Errors
    /// Invalid or missing source profile, or filesystem failure.
    pub async fn duplicate(
        &self,
        name: String,
    ) -> Result<ProfileCommit<DuplicateProfileResult>, AppError> {
        let name = ProfileStore::validate_profile_name(&name)?;
        self.commit(DuplicateProfile { name }, ProfileChangeReason::Duplicated)
            .await
    }

    /// Renames `old_name` to `new_name`, removes the launchers exported under
    /// the old name, rewrites the launcher display name and moves the
    /// last-used-profile setting. Returns whether a launcher was removed (the
    /// UI offers a re-export). Publishes `Renamed { from }` with the new name;
    /// renaming a profile to its own name changes nothing and publishes
    /// nothing.
    ///
    /// # Errors
    /// Invalid names, missing source, existing target or filesystem failure.
    pub async fn rename(
        &self,
        old_name: String,
        new_name: String,
    ) -> Result<ProfileCommit<bool>, AppError> {
        let old_name = ProfileStore::validate_profile_name(&old_name)?;
        let new_name = ProfileStore::validate_profile_name(&new_name)?;
        let reason = ProfileChangeReason::Renamed {
            from: old_name.clone(),
        };
        self.commit(RenameProfile { old_name, new_name }, reason)
            .await
    }

    /// Imports a legacy `.profile` file as a TOML profile named after its
    /// (trimmed) file stem. The returned profile has unknown keys cleared.
    /// Publishes `Imported`.
    ///
    /// # Errors
    /// `Validation` for a file stem that is not a valid profile name (before
    /// any side effect), unreadable legacy file or filesystem failure.
    pub async fn import_legacy(
        &self,
        path: PathBuf,
    ) -> Result<ProfileCommit<GameProfile>, AppError> {
        // The store names the profile after the validated file stem.
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                AppError::from(ProfileStoreError::InvalidName(path.display().to_string()))
            })?;
        let name = ProfileStore::validate_profile_name(stem)?;
        self.commit(
            ImportLegacyProfile { path, name },
            ProfileChangeReason::Imported,
        )
        .await
    }

    /// Serializes `profile` to a shareable TOML document whose comment header
    /// names `name`. Writes nothing.
    ///
    /// # Errors
    /// `Validation` when `name` is not a valid profile name (it is
    /// interpolated into the header, D12); `Internal` on serialization failure.
    pub async fn export_toml(
        &self,
        name: String,
        profile: GameProfile,
        include_hooks: bool,
    ) -> Result<String, AppError> {
        let name = ProfileStore::validate_profile_name(&name)?;
        profile_to_shareable_toml_with_options(
            &name,
            &profile,
            ShareableTomlOptions { include_hooks },
        )
        .map_err(|error| AppError::new(AppErrorKind::Internal, error.to_string()))
    }
}

/// Observe, revision and health invalidation for a profile written as `name`.
fn write_effects(
    name: &str,
    profile: GameProfile,
    sync_source: SyncSource,
    source_profile_id: Option<String>,
    revision_source: ConfigRevisionSource,
) -> Vec<Effect> {
    vec![
        Effect::ObserveWrite {
            name: name.to_string(),
            profile: profile.clone(),
            source: sync_source,
            source_profile_id,
        },
        Effect::Revision {
            name: name.to_string(),
            profile,
            source: revision_source,
            source_revision_id: None,
        },
        Effect::InvalidateHealth {
            name: name.to_string(),
        },
    ]
}

/// Holds a name already validated (trimmed) by [`ProfileWriter::save`].
struct SaveProfile {
    name: String,
    profile: GameProfile,
}

impl ProfileMutation for SaveProfile {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Saved)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        let Self { name, mut profile } = self;
        // BR-4: reject a malformed runtime Steam App ID before touching disk.
        validate_steam_app_id(profile.runtime.steam_app_id.trim()).map_err(|error| {
            AppError::validation(
                "steam_app_id",
                format!("Invalid Steam App ID in runtime section: {error}"),
            )
        })?;

        profile.clear_extra();
        if ctx.profiles.profile_exists(&name) {
            let existing = ctx.profiles.load(&name)?;
            profile.preserve_extra_from(&existing);
        } else {
            let settings = ctx.settings.load()?;
            apply_new_profile_defaults(&mut profile, &settings, ctx.metadata);
        }
        auto_import_custom_art(&name, &mut profile);

        ctx.profiles.save(&name, &profile)?;
        Ok(Persisted {
            output: (),
            effects: write_effects(
                &name,
                profile,
                SyncSource::AppWrite,
                None,
                ConfigRevisionSource::ManualSave,
            ),
            names: Some(vec![name]),
        })
    }
}

/// Holds a name already validated (trimmed) by [`ProfileWriter::delete`].
struct DeleteProfile {
    name: String,
}

impl ProfileMutation for DeleteProfile {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Deleted)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        let Self { name } = self;
        // Best-effort: the profile delete must succeed even if cleanup fails.
        if let Ok(profile) = ctx.profiles.load(&name) {
            if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
                tracing::warn!(%error, profile_name = %name, "launcher cleanup before profile delete failed");
            }
        }

        ctx.profiles.delete(&name)?;
        Ok(Persisted {
            output: (),
            // Health invalidation resolves the profile id by its current
            // name, so it runs before the soft delete.
            effects: vec![
                Effect::InvalidateHealth { name: name.clone() },
                Effect::ObserveDelete { name: name.clone() },
            ],
            names: Some(vec![name]),
        })
    }
}

/// Holds a name already validated (trimmed) by [`ProfileWriter::duplicate`].
struct DuplicateProfile {
    name: String,
}

impl ProfileMutation for DuplicateProfile {
    type Output = DuplicateProfileResult;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Duplicated)
    }

    fn persist(
        self,
        ctx: &CommitContext<'_>,
    ) -> Result<Persisted<DuplicateProfileResult>, AppError> {
        // Lineage is advisory: a failed lookup still duplicates.
        let source_profile_id = match ctx.metadata.lookup_profile_id(&self.name) {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!(%error, profile_name = %self.name, "source profile id lookup for duplicate failed");
                None
            }
        };

        let mut result = ctx.profiles.duplicate(&self.name)?;
        let effects = write_effects(
            &result.name,
            result.profile.clone(),
            SyncSource::AppDuplicate,
            source_profile_id,
            ConfigRevisionSource::ManualSave,
        );
        let names = Some(vec![result.name.clone()]);
        result.profile.clear_extra();
        Ok(Persisted {
            output: result,
            effects,
            names,
        })
    }
}

/// Holds names already validated (trimmed) by [`ProfileWriter::rename`].
struct RenameProfile {
    old_name: String,
    new_name: String,
}

impl ProfileMutation for RenameProfile {
    type Output = bool;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Renamed { from } if *from == self.old_name)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<bool>, AppError> {
        let Self { old_name, new_name } = self;
        // Loaded before the rename for launcher cleanup and the display name.
        let old_profile = ctx.profiles.load(&old_name).ok();

        ctx.profiles.rename(&old_name, &new_name)?;
        if old_name == new_name {
            return Ok(Persisted {
                output: false,
                effects: Vec::new(),
                names: None,
            });
        }

        // Best-effort: delete the old launcher files so the UI can re-export
        // them under the new name.
        let had_launcher = old_profile.as_ref().is_some_and(|profile| {
            match cleanup_launchers_for_profile_delete(&old_name, profile) {
                Ok(Some(result)) => result.script_deleted || result.desktop_entry_deleted,
                Ok(None) => false,
                Err(error) => {
                    tracing::warn!(%error, %old_name, %new_name, "launcher cleanup during profile rename failed");
                    false
                }
            }
        });

        let mut effects = vec![Effect::ObserveRename {
            old_name: old_name.clone(),
            new_name: new_name.clone(),
        }];
        if old_profile.is_some() {
            if let Some(profile) = rewrite_launcher_display_name(ctx.profiles, &new_name) {
                effects.push(Effect::Revision {
                    name: new_name.clone(),
                    profile,
                    source: ConfigRevisionSource::ManualSave,
                    source_revision_id: None,
                });
            }
        }
        effects.push(Effect::InvalidateHealth {
            name: new_name.clone(),
        });
        // Last: a poisoned settings mutex panics inside the guarded effects,
        // after the metadata steps, without failing the persisted rename.
        effects.push(Effect::MoveLastUsedProfile {
            old_name,
            new_name: new_name.clone(),
        });

        Ok(Persisted {
            output: had_launcher,
            effects,
            names: Some(vec![new_name]),
        })
    }
}

/// Points `steam.launcher.display_name` at the new name so future exports
/// use it. Returns the saved profile, or `None` (logged) when the update
/// could not be written.
fn rewrite_launcher_display_name(profiles: &ProfileStore, new_name: &str) -> Option<GameProfile> {
    let mut profile = match profiles.load(new_name) {
        Ok(profile) => profile,
        Err(error) => {
            tracing::warn!(%error, %new_name, "reload after profile rename failed; display name unchanged");
            return None;
        }
    };
    profile.steam.launcher.display_name = new_name.to_string();
    match profiles.save(new_name, &profile) {
        Ok(()) => Some(profile),
        Err(error) => {
            tracing::warn!(%error, %new_name, "display_name update after profile rename failed");
            None
        }
    }
}

/// Holds the profile name [`ProfileWriter::import_legacy`] validated from
/// the file stem; the store derives the same name.
struct ImportLegacyProfile {
    path: PathBuf,
    name: String,
}

impl ProfileMutation for ImportLegacyProfile {
    type Output = GameProfile;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Imported)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<GameProfile>, AppError> {
        let Self { path, name } = self;
        let mut profile = ctx.profiles.import_legacy(&path)?;
        let effects = write_effects(
            &name,
            profile.clone(),
            SyncSource::Import,
            None,
            ConfigRevisionSource::Import,
        );
        profile.clear_extra();
        Ok(Persisted {
            output: profile,
            effects,
            names: Some(vec![name]),
        })
    }
}
