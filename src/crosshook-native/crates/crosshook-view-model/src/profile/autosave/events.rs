//! Input events and intents accepted by [`super::ProfileAutosave`].

use crate::ServiceError;

use super::state::{AutosaveInstant, AutosaveScope, AutosaveSection, RevisionHash, SaveId};

/// Reason profiles changed on the `crosshook-app` event bus.
///
/// Mirrors `crosshook_app::ProfileChangeReason` without pulling `crosshook-app`
/// or `tokio` into the default build.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProfileChangeKind {
    /// Profile content was written.
    Saved,
    /// Profile was deleted.
    Deleted,
    /// Profile was renamed from `from`.
    Renamed {
        /// Previous profile name.
        from: String,
    },
    /// Profile was duplicated.
    Duplicated,
    /// Metadata-only favorite toggle (never affects profile contents).
    FavoriteToggled,
    /// A config revision was rolled back.
    RolledBack,
    /// A bundled or manual optimization preset was applied.
    PresetApplied,
    /// Profiles were imported.
    Imported,
    /// Proton migrations were applied.
    Migrated,
    /// Subscriber fell behind on the event bus and must resync.
    Resync,
}

/// Notification that a profile changed on disk (from `ProfilesChanged` or `Resync`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalChange {
    /// Affected profile name; `None` represents a conservative broadcast resync.
    pub profile_name: Option<String>,
    /// Why the change happened.
    pub reason: ProfileChangeKind,
    /// Revision hash of the external state, when known.
    pub revision: Option<RevisionHash>,
    /// Write request id that originated this notification, when known by the bridge.
    ///
    /// When `Some`, [`super::ProfileAutosave`] suppresses matching self-write
    /// echoes by id even if saving normalized the revision hash.
    pub origin_save_id: Option<SaveId>,
}

/// Explicit user choice resolving an [`super::AutosaveState::Conflict`].
///
/// The reducer never auto-merges conflicting edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictResolution {
    /// Overwrite disk with the local draft (`FullProfile` save).
    KeepMine,
    /// Discard local edits and reload the on-disk profile.
    ReloadTheirs,
}

/// Events processed by [`super::ProfileAutosave::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutosaveEvent {
    /// Profile finished loading or reloading from disk with `revision`.
    Loaded {
        /// Loaded scope.
        scope: AutosaveScope,
        /// Revision hash of the loaded profile.
        revision: RevisionHash,
    },
    /// Active profile selection, draft name, existence or launch method changed.
    ScopeChanged(AutosaveScope),
    /// User edited `section`, producing draft snapshot `revision`.
    Edit {
        /// Edited section.
        section: AutosaveSection,
        /// Updated revision hash (full profile or section fingerprint).
        revision: RevisionHash,
    },
    /// Scheduled timer fired at `now`.
    Tick,
    /// Immediate flush requested (route navigation, card switch, window close
    /// or editor unmount).
    Flush,
    /// User manually triggered retry from an error state.
    RetryNow,
    /// Bridge finished executing [`super::AutosaveEffect::Save`].
    SaveSucceeded {
        /// Request id from [`super::SaveRequest::id`].
        id: SaveId,
        /// Revision hash committed to disk.
        committed_rev: RevisionHash,
    },
    /// Bridge failed executing [`super::AutosaveEffect::Save`].
    SaveFailed {
        /// Request id from [`super::SaveRequest::id`].
        id: SaveId,
        /// Sanitized error from `crosshook-app`.
        error: ServiceError,
    },
    /// A `ProfilesChanged` or `Resync` event arrived from the bus.
    ExternalChange(ExternalChange),
    /// User resolved an active conflict.
    ResolveConflict(ConflictResolution),
}

/// [`crate::ViewModel::Intent`] pairing an [`AutosaveEvent`] with the injected
/// monotonic timestamp `now`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutosaveIntent {
    /// Domain event to apply.
    pub event: AutosaveEvent,
    /// Current monotonic timestamp.
    pub now: AutosaveInstant,
}

impl AutosaveIntent {
    /// Pairs `event` with `now`.
    #[must_use]
    pub const fn at(now: AutosaveInstant, event: AutosaveEvent) -> Self {
        Self { event, now }
    }
}
