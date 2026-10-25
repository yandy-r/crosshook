//! Launch optimization, MangoHud, gamescope, trainer-gamescope and
//! command-argument section save facades.
//!
//! Each save validates and trims the profile name, loads the profile,
//! applies one core mutation rule and persists the TOML once, then records
//! metadata sync and a config revision and publishes
//! `ProfileChangeReason::Saved` with the trimmed name.

use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};
use crosshook_core::profile::{
    GameProfile, GamescopeConfig, MangoHudConfig, ProfileStore, ProfileStoreError,
};

use super::types::{
    CommandArgumentsPayload, Effect, LaunchOptimizationsPayload, Persisted, ProfileCommit,
    ProfileMutation,
};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, ProfileChangeReason};

impl ProfileWriter {
    /// Replaces the launch optimizations of profile `name` (trimmed), or
    /// switches to an existing named preset when
    /// `optimizations.switch_active_preset` is set.
    ///
    /// # Errors
    /// `Validation` for a blank or invalid name or unknown optimization ids,
    /// `NotFound` for a missing profile or preset, `Io` when the TOML write
    /// fails.
    pub async fn save_launch_optimizations(
        &self,
        name: String,
        optimizations: LaunchOptimizationsPayload,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = required_profile_name("name", &name)?;
        self.commit(
            LaunchSelectionSave {
                name,
                selection: LaunchSelection::Optimizations(optimizations),
            },
            ProfileChangeReason::Saved,
        )
        .await
    }

    /// Replaces `launch.command_arguments` of profile `name` (trimmed),
    /// validated against `resolved_launch_method` when it names a known
    /// method, otherwise against the profile's own method.
    ///
    /// # Errors
    /// `Validation` for a blank or invalid name, unknown or conflicting
    /// argument ids, or token limits; `NotFound` for a missing profile; `Io`
    /// on write failure.
    pub async fn save_command_arguments(
        &self,
        name: String,
        command_arguments: CommandArgumentsPayload,
        resolved_launch_method: Option<String>,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = required_profile_name("name", &name)?;
        self.commit(
            LaunchSelectionSave {
                name,
                selection: LaunchSelection::CommandArguments {
                    payload: command_arguments,
                    resolved_launch_method,
                },
            },
            ProfileChangeReason::Saved,
        )
        .await
    }

    /// Replaces `launch.mangohud` of profile `name` (trimmed).
    ///
    /// # Errors
    /// `Validation` for a blank or invalid name, `NotFound` for a missing
    /// profile, `Io` when the TOML write fails.
    pub async fn save_mangohud_config(
        &self,
        name: String,
        config: MangoHudConfig,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = required_profile_name("name", &name)?;
        self.commit(
            SectionSave {
                name,
                section: Section::MangoHud(config),
            },
            ProfileChangeReason::Saved,
        )
        .await
    }

    /// Replaces `launch.gamescope` of profile `name` (trimmed).
    ///
    /// # Errors
    /// Same as [`Self::save_mangohud_config`].
    pub async fn save_gamescope_config(
        &self,
        name: String,
        config: GamescopeConfig,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = required_profile_name("name", &name)?;
        self.commit(
            SectionSave {
                name,
                section: Section::Gamescope(config),
            },
            ProfileChangeReason::Saved,
        )
        .await
    }

    /// Replaces `launch.trainer_gamescope` of profile `name` (trimmed).
    ///
    /// # Errors
    /// Same as [`Self::save_mangohud_config`].
    pub async fn save_trainer_gamescope_config(
        &self,
        name: String,
        config: GamescopeConfig,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = required_profile_name("name", &name)?;
        self.commit(
            SectionSave {
                name,
                section: Section::TrainerGamescope(config),
            },
            ProfileChangeReason::Saved,
        )
        .await
    }
}

/// Validates `raw` with [`ProfileStore::validate_profile_name`] and returns
/// the trimmed name; failures are `Validation { field }` (blank names get a
/// "required" message).
pub(super) fn required_profile_name(field: &'static str, raw: &str) -> Result<String, AppError> {
    if raw.trim().is_empty() {
        return Err(AppError::validation(field, "profile name is required"));
    }
    ProfileStore::validate_profile_name(raw)
        .map_err(|error: ProfileStoreError| AppError::validation(field, error.to_string()))
}

/// Metadata sync (`AppWrite`) and a config revision of `profile` as saved.
pub(super) fn write_effects(
    name: &str,
    profile: GameProfile,
    source: ConfigRevisionSource,
) -> Vec<Effect> {
    vec![
        Effect::ObserveWrite {
            name: name.to_string(),
            profile: profile.clone(),
            source: SyncSource::AppWrite,
            source_profile_id: None,
        },
        Effect::Revision {
            name: name.to_string(),
            profile,
            source,
            source_revision_id: None,
        },
    ]
}

/// Launch selection replaced by a core mutation rule.
enum LaunchSelection {
    Optimizations(LaunchOptimizationsPayload),
    CommandArguments {
        payload: CommandArgumentsPayload,
        resolved_launch_method: Option<String>,
    },
}

impl LaunchSelection {
    fn revision_source(&self) -> ConfigRevisionSource {
        match self {
            Self::Optimizations(_) => ConfigRevisionSource::LaunchOptimizationSave,
            Self::CommandArguments { .. } => ConfigRevisionSource::LaunchCommandArgumentsSave,
        }
    }

    /// Load → apply → save, writing the TOML exactly once.
    fn write(self, ctx: &CommitContext<'_>, name: &str) -> Result<(), ProfileStoreError> {
        match self {
            Self::Optimizations(payload) => {
                let mut profile = ctx.profiles.load(name)?;
                profile.apply_launch_optimizations(
                    payload.enabled_option_ids,
                    payload.switch_active_preset,
                )?;
                ctx.profiles.save(name, &profile)
            }
            Self::CommandArguments {
                payload,
                resolved_launch_method,
            } => {
                let method = resolved_launch_method.as_deref();
                let result = ctx.profiles.load(name).and_then(|mut profile| {
                    profile.apply_command_arguments(
                        payload.enabled_argument_ids,
                        payload.custom_args,
                        method,
                    )?;
                    ctx.profiles.save(name, &profile)
                });
                if let Err(error) = &result {
                    tracing::warn!(
                        profile = name,
                        resolved_launch_method = method.unwrap_or("<none>"),
                        %error,
                        "profile_save_command_arguments rejected by store"
                    );
                }
                result
            }
        }
    }
}

/// Optimization or command-argument save of one profile.
struct LaunchSelectionSave {
    name: String,
    selection: LaunchSelection,
}

impl ProfileMutation for LaunchSelectionSave {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Saved)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        let Self { name, selection } = self;
        let source = selection.revision_source();
        selection.write(ctx, &name)?;
        // Metadata sync and the revision describe the profile as reloaded
        // from disk; the write already succeeded, so a failed reload only
        // skips them.
        let effects = match ctx.profiles.load(&name) {
            Ok(updated) => write_effects(&name, updated, source),
            Err(error) => {
                tracing::warn!(
                    %error,
                    profile_name = %name,
                    "reload after launch selection save failed; skipping metadata sync and config revision"
                );
                Vec::new()
            }
        };
        Ok(Persisted {
            output: (),
            effects,
            names: Some(vec![name]),
        })
    }
}

/// One launch section replaced wholesale.
enum Section {
    MangoHud(MangoHudConfig),
    Gamescope(GamescopeConfig),
    TrainerGamescope(GamescopeConfig),
}

impl Section {
    fn apply(self, profile: &mut GameProfile) {
        match self {
            Self::MangoHud(config) => profile.launch.mangohud = config,
            Self::Gamescope(config) => profile.launch.gamescope = config,
            Self::TrainerGamescope(config) => profile.launch.trainer_gamescope = config,
        }
    }
}

/// MangoHud, gamescope or trainer-gamescope save of one profile.
struct SectionSave {
    name: String,
    section: Section,
}

impl ProfileMutation for SectionSave {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Saved)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        let Self { name, section } = self;
        let mut profile = ctx.profiles.load(&name)?;
        let existing = profile.clone();
        section.apply(&mut profile);
        // Unknown keys come from disk, never from the request.
        profile.clear_extra();
        profile.preserve_extra_from(&existing);
        ctx.profiles.save(&name, &profile)?;
        Ok(Persisted {
            output: (),
            effects: write_effects(&name, profile, ConfigRevisionSource::ManualSave),
            names: Some(vec![name]),
        })
    }
}
