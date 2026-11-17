//! The private commit pipeline every mutating facade funnels through.
//!
//! `commit` checks the mutation against its reason, then on the blocking pool
//! under the writer lock: primary persistence → advisory effects
//! (observe → revision → health invalidation, fail-soft, D5/D6) → publish
//!. The event is published inside the closure, so a dropped caller
//! future cannot lose the notification for a write that persisted; a panic
//! inside the advisory effects is caught and logged so it cannot suppress
//! the event either.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::PoisonError;

use crosshook_core::metadata::{MetadataStore, ProfileLaunchPresetOrigin};
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;

use super::revisions::{
    capture_config_revision, resolve_config_history_max_revisions, RevisionOutcome,
};
use super::run_blocking;
use super::types::{Effect, Persisted, ProfileCommit, ProfileMutation};
use super::ProfileWriter;
use crate::{AppError, AppErrorKind, AppEvent, ProfileChangeReason};

/// Stores available to [`ProfileMutation::persist`]; borrowed for the
/// duration of one commit, under the writer lock.
pub(crate) struct CommitContext<'a> {
    /// Profile TOML store.
    pub(crate) profiles: &'a ProfileStore,
    /// Metadata store (may be disabled).
    pub(crate) metadata: &'a MetadataStore,
    /// Settings store.
    pub(crate) settings: &'a SettingsStore,
}

impl ProfileWriter {
    /// Runs `mutation` through the commit pipeline and publishes
    /// `AppEvent::ProfilesChanged { reason, names }` once it persisted.
    ///
    /// # Errors
    /// `Internal` when `mutation` does not accept `reason` (nothing runs);
    /// otherwise the error returned by [`ProfileMutation::persist`] (no
    /// effects, no event). Advisory effect failures are logged, not errors.
    pub(crate) async fn commit<M: ProfileMutation>(
        &self,
        mutation: M,
        reason: ProfileChangeReason,
    ) -> Result<ProfileCommit<M::Output>, AppError> {
        if !mutation.accepts(&reason) {
            return Err(AppError::new(
                AppErrorKind::Internal,
                "profile mutation does not match its change reason",
            ));
        }
        let writer = self.clone();
        run_blocking(move || {
            // The guard protects `()`: poisoning carries no invariant, so a
            // panicked writer must not wedge every later save.
            let _guard = writer
                .write_lock
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let ctx = CommitContext {
                profiles: &writer.profiles,
                metadata: &writer.metadata,
                settings: &writer.settings,
            };
            let Persisted {
                output,
                effects,
                names,
            } = mutation.persist(&ctx)?;
            let new_revision_id = run_effects_guarded(&ctx, effects);
            if let Some(names) = names {
                writer
                    .events
                    .publish(AppEvent::ProfilesChanged { reason, names });
            }
            Ok(ProfileCommit {
                output,
                new_revision_id,
            })
        })
        .await
    }
}

/// Runs [`run_effects`]; a panic is logged and the remaining effects are
/// skipped, so the primary write is still reported as a success and its event
/// is still published.
fn run_effects_guarded(ctx: &CommitContext<'_>, effects: Vec<Effect>) -> Option<i64> {
    // Unwind safety: effects only touch the stores through their own locks
    // (recovered or reported on poison); nothing observed here is left
    // half-updated by an unwind.
    catch_unwind(AssertUnwindSafe(|| run_effects(ctx, effects))).unwrap_or_else(|_| {
        tracing::warn!("advisory profile effect panicked; remaining effects skipped");
        None
    })
}

/// Runs advisory effects in order and returns the last inserted revision id.
/// Failed steps are logged and skipped; this never fails the commit.
fn run_effects(ctx: &CommitContext<'_>, mut effects: Vec<Effect>) -> Option<i64> {
    let mut new_revision_id = None;
    if !ctx.metadata.is_available() {
        let before = effects.len();
        effects.retain(|effect| !effect.needs_metadata());
        let skipped_effects = before - effects.len();
        if skipped_effects > 0 {
            tracing::warn!(
                skipped_effects,
                "metadata store unavailable; profile saved without metadata sync, config revision or health invalidation"
            );
        }
    }

    let mut max_revisions = None;
    for effect in effects {
        match effect {
            Effect::ObserveWrite {
                name,
                profile,
                source,
                source_profile_id,
            } => {
                let path = ctx.profiles.base_path.join(format!("{name}.toml"));
                if let Err(error) = ctx.metadata.observe_profile_write(
                    &name,
                    &profile,
                    &path,
                    source,
                    source_profile_id.as_deref(),
                ) {
                    tracing::warn!(%error, profile_name = %name, "metadata sync after profile write failed");
                }
            }
            Effect::ObserveRename { old_name, new_name } => {
                let old_path = ctx.profiles.base_path.join(format!("{old_name}.toml"));
                let new_path = ctx.profiles.base_path.join(format!("{new_name}.toml"));
                if let Err(error) = ctx
                    .metadata
                    .observe_profile_rename(&old_name, &new_name, &old_path, &new_path)
                {
                    tracing::warn!(%error, %old_name, %new_name, "metadata sync after profile rename failed");
                }
            }
            Effect::ObserveDelete { name } => {
                if let Err(error) = ctx.metadata.observe_profile_delete(&name) {
                    tracing::warn!(%error, profile_name = %name, "metadata sync after profile delete failed");
                }
            }
            Effect::Revision {
                name,
                profile,
                source,
                source_revision_id,
            } => {
                let max = *max_revisions
                    .get_or_insert_with(|| resolve_config_history_max_revisions(ctx.settings));
                match capture_config_revision(
                    &name,
                    &profile,
                    source,
                    source_revision_id,
                    ctx.metadata,
                    max,
                ) {
                    RevisionOutcome::Inserted(id) => new_revision_id = Some(id),
                    RevisionOutcome::Deduped | RevisionOutcome::Skipped => {}
                }
            }
            Effect::InvalidateHealth { name } => {
                invalidate_health(ctx.metadata, &name);
            }
            Effect::UpsertPresetOrigin {
                name,
                preset_name,
                origin,
                source_bundled_preset_id,
            } => {
                upsert_preset_origin(
                    ctx.metadata,
                    &name,
                    &preset_name,
                    origin,
                    source_bundled_preset_id.as_deref(),
                );
            }
            Effect::SeedVersionSnapshot {
                name,
                steam_app_id,
                trainer_version,
                human_game_ver,
            } => {
                seed_version_snapshot(
                    ctx.metadata,
                    &name,
                    &steam_app_id,
                    trainer_version.as_deref(),
                    human_game_ver.as_deref(),
                );
            }
            Effect::MoveLastUsedProfile { old_name, new_name } => {
                move_last_used_profile(ctx.settings, &old_name, &new_name);
            }
            #[cfg(test)]
            Effect::Panic => panic!("test effect panic"),
        }
    }
    new_revision_id
}

/// Moves `last_used_profile` to `new_name` when it pointed at `old_name`;
/// leaves the settings file untouched otherwise. Fail-soft: errors are
/// logged; a poisoned settings mutex panics into the effect guard.
fn move_last_used_profile(settings: &SettingsStore, old_name: &str, new_name: &str) {
    let outcome = settings.update(|data| {
        if data.last_used_profile.trim() == old_name {
            data.last_used_profile = new_name.to_string();
            Ok(())
        } else {
            // Not the renamed profile: skip the write.
            Err(())
        }
    });
    if let Err(error) = outcome {
        tracing::warn!(%error, %old_name, %new_name, "settings update after profile rename failed");
    }
}

/// Deletes the cached health row of the profile currently named `name`.
/// Lookup and delete failures are logged; a profile without metadata has
/// nothing cached.
fn invalidate_health(metadata: &MetadataStore, name: &str) {
    let profile_id = match metadata.lookup_profile_id(name) {
        Ok(Some(id)) => id,
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%error, profile_name = %name, "profile id lookup for health invalidation failed");
            return;
        }
    };
    if let Err(error) = metadata.invalidate_health_snapshot(&profile_id) {
        tracing::warn!(%error, profile_name = %name, "health snapshot invalidation failed");
    }
}

/// Upserts the launch preset origin row of the profile currently named
/// `name`. Lookup and upsert failures are logged; a profile that is still not
/// indexed is skipped without a warning.
fn upsert_preset_origin(
    metadata: &MetadataStore,
    name: &str,
    preset_name: &str,
    origin: ProfileLaunchPresetOrigin,
    source_bundled_preset_id: Option<&str>,
) {
    let profile_id = match metadata.lookup_profile_id(name) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::debug!(profile_name = %name, "profile_id not yet indexed — skipping preset metadata upsert");
            return;
        }
        Err(error) => {
            tracing::warn!(%error, profile_name = %name, "lookup_profile_id failed during preset metadata upsert");
            return;
        }
    };
    if let Err(error) = metadata.upsert_profile_launch_preset_metadata(
        &profile_id,
        preset_name,
        origin,
        source_bundled_preset_id,
    ) {
        tracing::warn!(
            %error,
            profile_name = %name,
            origin = origin.as_str(),
            "failed to upsert preset metadata row"
        );
    }
}

/// Upserts the display-only (`untracked`) version snapshot of the profile
/// currently named `name` (BR-8/W3). Lookup and upsert failures are logged.
fn seed_version_snapshot(
    metadata: &MetadataStore,
    name: &str,
    steam_app_id: &str,
    trainer_version: Option<&str>,
    human_game_ver: Option<&str>,
) {
    let profile_id = match metadata.lookup_profile_id(name) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!(profile_name = %name, "version snapshot seed skipped: profile not found in metadata");
            return;
        }
        Err(error) => {
            tracing::warn!(%error, profile_name = %name, "version snapshot seed skipped: profile id lookup failed");
            return;
        }
    };
    if let Err(error) = metadata.upsert_version_snapshot(
        &profile_id,
        steam_app_id,
        None,
        trainer_version,
        None,
        human_game_ver,
        "untracked",
    ) {
        tracing::warn!(%error, profile_name = %name, "version snapshot seed after community import failed");
    }
}
