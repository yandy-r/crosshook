//! Profile service DTOs and the commit-pipeline contract.
//!
//! The DTOs keep the exact serde shapes of the Tauri commands they replace.
//! [`ProfileMutation`], [`Persisted`] and [`Effect`] are the crate-private
//! contract every mutating facade implements; [`super::writer`] runs them.

use std::path::PathBuf;

use crosshook_core::metadata::{ConfigRevisionSource, ProfileLaunchPresetOrigin, SyncSource};
use crosshook_core::profile::GameProfile;
use serde::{Deserialize, Serialize};

use super::writer::CommitContext;
use crate::{AppError, ProfileChangeReason};

/// Library card summary for one profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    /// Profile file stem.
    pub name: String,
    /// Effective game display name.
    pub game_name: String,
    /// Steam App ID used for art lookup.
    pub steam_app_id: String,
    /// Trimmed custom cover art path, if any.
    pub custom_cover_art_path: Option<String>,
    /// Trimmed custom portrait art path, if any.
    pub custom_portrait_art_path: Option<String>,
    /// Effective `launch.network_isolation` (default true) for Flatpak capability badges.
    pub network_isolation: bool,
}

/// Launch optimization selection sent by the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsPayload {
    /// Enabled optimization catalog ids.
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
    /// When set, selects that named preset from `launch.presets` and ignores `enabled_option_ids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_active_preset: Option<String>,
}

/// Command-argument selection sent by the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandArgumentsPayload {
    /// Enabled curated argument ids.
    #[serde(
        rename = "enabled_argument_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_argument_ids: Vec<String>,
    /// Custom argv tokens.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_args: Vec<String>,
}

/// Bundled optimization preset as listed to the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundledOptimizationPresetDto {
    /// Stable bundled preset id.
    pub preset_id: String,
    /// Human-readable name.
    pub display_name: String,
    /// GPU vendor the preset targets.
    pub vendor: String,
    /// Preset mode (e.g. performance/quality).
    pub mode: String,
    /// Optimization ids the preset enables.
    pub enabled_option_ids: Vec<String>,
    /// Optimization catalog version the preset was built against.
    pub catalog_version: i64,
}

/// A profile the core Lutris importer already wrote; `record_imports` runs
/// only the shared post-persistence effects for it.
#[derive(Debug, Clone)]
pub struct ImportedProfileWrite {
    /// Profile name the importer wrote.
    pub name: String,
    /// Profile as written.
    pub profile: GameProfile,
    /// Path of the written TOML.
    pub path: PathBuf,
}

/// Advisory step a commit skipped or failed after the primary write succeeded.
/// Every warning is also logged with `tracing::warn!`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitWarning {
    /// Metadata store unavailable: metadata sync, config revision and health
    /// invalidation were all skipped.
    MetadataUnavailable,
    /// Metadata sync (write, rename or delete observation) failed.
    ObserveFailed,
    /// Config revision capture failed. Hash dedupe is not a warning.
    RevisionSkipped,
    /// Cached health snapshot invalidation failed.
    HealthInvalidationFailed,
    /// Launch preset origin row upsert failed.
    PresetOriginFailed,
}

/// Result of a committed profile mutation.
#[derive(Debug, Clone)]
pub struct ProfileCommit<T> {
    /// Facade-specific result (the unchanged IPC response value).
    pub output: T,
    /// Id of the last config revision inserted; `None` when deduped or skipped.
    pub new_revision_id: Option<i64>,
    /// Advisory steps that were skipped or failed.
    pub warnings: Vec<CommitWarning>,
}

impl<T> ProfileCommit<T> {
    /// Drops the commit metadata, keeping the IPC response value.
    pub fn into_output(self) -> T {
        self.output
    }
}

/// One operation run by `ProfileWriter::commit`: validate → load → mutate →
/// persist exactly once, then describe the advisory effects to run.
///
/// Implementations live next to the facade that builds them; only facades
/// call `commit`, so callers cannot pick a mismatched reason.
pub(crate) trait ProfileMutation: Send + 'static {
    /// Facade result carried in [`ProfileCommit::output`].
    type Output: Send + 'static;

    /// Whether this mutation may publish under `reason`. `commit` rejects a
    /// mismatch before any side effect.
    fn accepts(&self, reason: &ProfileChangeReason) -> bool;

    /// Performs the primary persistence. Runs under the writer lock on the
    /// blocking pool. Must not run metadata effects itself; return them.
    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<Self::Output>, AppError>;
}

/// Primary-persistence outcome returned by [`ProfileMutation::persist`].
pub(crate) struct Persisted<T> {
    /// Facade result.
    pub(crate) output: T,
    /// Advisory effects, run in order, fail-soft.
    pub(crate) effects: Vec<Effect>,
    /// Names to publish; `None` when nothing changed (no event).
    pub(crate) names: Option<Vec<String>>,
}

/// Advisory post-persistence step, executed in list order by the writer.
pub(crate) enum Effect {
    /// `MetadataStore::observe_profile_write` for `<profiles>/<name>.toml`.
    ObserveWrite {
        name: String,
        profile: GameProfile,
        source: SyncSource,
        source_profile_id: Option<String>,
    },
    /// `MetadataStore::observe_profile_rename` (keeps the stable profile id).
    ObserveRename { old_name: String, new_name: String },
    /// `MetadataStore::observe_profile_delete` (soft delete).
    ObserveDelete { name: String },
    /// Config revision snapshot of `profile` (serialized as given, not storage form).
    Revision {
        name: String,
        profile: GameProfile,
        source: ConfigRevisionSource,
        source_revision_id: Option<i64>,
    },
    /// Drops the cached health snapshot of the profile currently named `name`.
    /// Order it before `ObserveDelete` for deletes.
    InvalidateHealth { name: String },
    /// `MetadataStore::upsert_profile_launch_preset_metadata` for the profile
    /// currently named `name`. Order it after `ObserveWrite` so a profile not
    /// yet indexed gets its id first.
    UpsertPresetOrigin {
        name: String,
        preset_name: String,
        origin: ProfileLaunchPresetOrigin,
        source_bundled_preset_id: Option<String>,
    },
}
