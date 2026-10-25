//! Bundled and manual optimization preset facades.
//!
//! Applying a preset writes `[launch.presets.<key>]`, activates it, records
//! metadata sync, a `PresetApply` revision and the preset-origin row, then
//! publishes `ProfileChangeReason::PresetApplied`.

use crosshook_core::metadata::{
    BundledOptimizationPresetRow, ConfigRevisionSource, MetadataStore, ProfileLaunchPresetOrigin,
};
use crosshook_core::profile::{bundled_optimization_preset_toml_key, GameProfile};

use super::run_blocking;
use super::sections::{required_profile_name, write_effects};
use super::types::{
    BundledOptimizationPresetDto, Effect, Persisted, ProfileCommit, ProfileMutation,
};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, AppErrorKind, ProfileChangeReason};

impl ProfileWriter {
    /// Lists the bundled optimization presets seeded in the metadata store.
    ///
    /// # Errors
    /// `Unavailable` when the metadata store is unavailable, `Internal` when a
    /// stored option list is corrupt or the query fails.
    pub async fn list_bundled_optimization_presets(
        &self,
    ) -> Result<Vec<BundledOptimizationPresetDto>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            require_metadata(&metadata, "list")?;
            metadata
                .list_bundled_optimization_presets()?
                .into_iter()
                .map(bundled_row_to_dto)
                .collect()
        })
        .await
    }

    /// Materializes bundled preset `preset_id` (trimmed) into profile `name`
    /// (trimmed) as `bundled/<preset_id>` and activates it. Returns the saved
    /// profile without unknown keys.
    ///
    /// # Errors
    /// `Unavailable` when the metadata store is unavailable, `Validation` for
    /// a blank or invalid name or a blank preset id, `NotFound` for an
    /// unknown preset or missing profile, `Io` when the TOML write fails.
    pub async fn apply_bundled_optimization_preset(
        &self,
        name: String,
        preset_id: String,
    ) -> Result<ProfileCommit<GameProfile>, AppError> {
        require_metadata(&self.metadata, "apply")?;
        let name = required_profile_name("name", &name)?;
        let preset_id = preset_id.trim();
        if preset_id.is_empty() {
            return Err(AppError::validation("preset_id", "preset_id is required"));
        }
        self.commit(
            PresetApply {
                name,
                preset: Preset::Bundled {
                    preset_id: preset_id.to_string(),
                },
            },
            ProfileChangeReason::PresetApplied,
        )
        .await
    }

    /// Stores `enabled_option_ids` in profile `name` (trimmed) as user preset
    /// `preset_name` (trimmed) and activates it. Returns the saved profile
    /// without unknown keys. Works without metadata; the preset-origin row is
    /// then skipped.
    ///
    /// # Errors
    /// `Validation` for a blank or invalid name, a blank or reserved preset
    /// name, or unknown option ids; `NotFound` for a missing profile; `Io` on
    /// write failure.
    pub async fn save_manual_optimization_preset(
        &self,
        name: String,
        preset_name: String,
        enabled_option_ids: Vec<String>,
    ) -> Result<ProfileCommit<GameProfile>, AppError> {
        let name = required_profile_name("name", &name)?;
        let preset_name = preset_name.trim();
        if preset_name.is_empty() {
            return Err(AppError::validation(
                "preset_name",
                "preset name must not be empty",
            ));
        }
        self.commit(
            PresetApply {
                name,
                preset: Preset::Manual {
                    preset_name: preset_name.to_string(),
                    enabled_option_ids,
                },
            },
            ProfileChangeReason::PresetApplied,
        )
        .await
    }
}

/// Bundled presets live only in the metadata store (D6: fail closed).
fn require_metadata(metadata: &MetadataStore, action: &str) -> Result<(), AppError> {
    if metadata.is_available() {
        Ok(())
    } else {
        Err(AppError::new(
            AppErrorKind::Unavailable,
            format!("metadata store is unavailable — cannot {action} bundled presets"),
        ))
    }
}

fn bundled_row_to_dto(
    row: BundledOptimizationPresetRow,
) -> Result<BundledOptimizationPresetDto, AppError> {
    let enabled_option_ids = parse_option_ids(&row)?;
    Ok(BundledOptimizationPresetDto {
        preset_id: row.preset_id,
        display_name: row.display_name,
        vendor: row.vendor,
        mode: row.mode,
        enabled_option_ids,
        catalog_version: row.catalog_version,
    })
}

fn parse_option_ids(row: &BundledOptimizationPresetRow) -> Result<Vec<String>, AppError> {
    serde_json::from_str(&row.option_ids_json).map_err(|error| {
        AppError::new(
            AppErrorKind::Internal,
            format!(
                "corrupt bundled preset {} option list: {error}",
                row.preset_id
            ),
        )
    })
}

/// Preset written into the profile.
enum Preset {
    Bundled {
        preset_id: String,
    },
    Manual {
        preset_name: String,
        enabled_option_ids: Vec<String>,
    },
}

/// Bundled or manual preset application to one profile.
struct PresetApply {
    name: String,
    preset: Preset,
}

impl ProfileMutation for PresetApply {
    type Output = GameProfile;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::PresetApplied)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<GameProfile>, AppError> {
        let Self { name, preset } = self;
        let (preset_name, origin, source_bundled_preset_id, saved) = match preset {
            Preset::Bundled { preset_id } => {
                let row = ctx
                    .metadata
                    .get_bundled_optimization_preset(&preset_id)?
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("unknown bundled preset: {preset_id}"),
                        )
                    })?;
                let enabled_option_ids = parse_option_ids(&row)?;
                let toml_key = bundled_optimization_preset_toml_key(&row.preset_id);
                let mut profile = ctx.profiles.load(&name)?;
                profile.materialize_launch_optimization_preset(
                    &toml_key,
                    enabled_option_ids,
                    true,
                )?;
                ctx.profiles.save(&name, &profile)?;
                (
                    toml_key,
                    ProfileLaunchPresetOrigin::Bundled,
                    Some(row.preset_id),
                    profile,
                )
            }
            Preset::Manual {
                preset_name,
                enabled_option_ids,
            } => {
                let mut profile = ctx.profiles.load(&name)?;
                profile.apply_manual_optimization_preset(&preset_name, enabled_option_ids)?;
                ctx.profiles.save(&name, &profile)?;
                (preset_name, ProfileLaunchPresetOrigin::User, None, profile)
            }
        };

        // The TOML is written: a failed reload falls back to the profile
        // just saved so effects and the event still run (D4).
        let updated = ctx.profiles.load(&name).unwrap_or_else(|error| {
            tracing::warn!(
                %error,
                profile_name = %name,
                "reload after preset apply failed; using the saved in-memory profile"
            );
            saved
        });
        let mut output = updated.clone();
        output.clear_extra();
        // Origin after observe, so a profile not yet indexed gets its id first.
        let mut effects = write_effects(&name, updated, ConfigRevisionSource::PresetApply);
        effects.push(Effect::UpsertPresetOrigin {
            name: name.clone(),
            preset_name,
            origin,
            source_bundled_preset_id,
        });
        Ok(Persisted {
            output,
            effects,
            names: Some(vec![name]),
        })
    }
}
