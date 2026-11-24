//! Effects emitted by [`super::ProfileAutosave`] for the UI bridge to run.

use std::collections::BTreeSet;

use super::events::ProfileChangeKind;
use super::policy::AutosaveStatus;
use super::state::{AutosaveInstant, AutosaveSection, RevisionHash, SaveId, SaveTarget};

/// Persistence command emitted when debounced or flushed edits are ready to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRequest {
    /// Unique operation id; the bridge must echo this in `SaveSucceeded` /
    /// `SaveFailed`.
    pub id: SaveId,
    /// Trimmed target profile name.
    pub profile_name: String,
    /// Which `ProfileWriter` path to call.
    pub target: SaveTarget,
    /// Sections covered by this write.
    pub sections: BTreeSet<AutosaveSection>,
    /// Revision hash of the draft being written (used for self-write
    /// suppression).
    pub revision: RevisionHash,
}

/// Context surfaced when an external disk change collides with unsaved local
/// edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictDetails {
    /// Conflicting profile name.
    pub profile_name: String,
    /// Latest local draft revision.
    pub local: RevisionHash,
    /// Remote revision reported on the change event, if known.
    pub remote: Option<RevisionHash>,
    /// Local sections modified or in flight when the conflict happened.
    pub sections: BTreeSet<AutosaveSection>,
    /// Underlying change reason.
    pub reason: ProfileChangeKind,
}

/// Plain-data effects returned by [`super::ProfileAutosave::apply`] and
/// [`crate::ViewModel::reduce`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutosaveEffect {
    /// Arm (or reschedule) the single timer to fire [`super::AutosaveEvent::Tick`]
    /// at `at`. Never emitted when the reducer is idle.
    ScheduleTick(AutosaveInstant),
    /// Execute a profile or section save via `ProfileWriter`.
    Save(SaveRequest),
    /// Prompt the user to resolve a conflict (`KeepMine` / `ReloadTheirs`).
    ShowConflict(ConflictDetails),
    /// Reload the profile from disk (external change while idle, or user chose
    /// `ReloadTheirs`).
    Reload {
        /// Trimmed profile name to reload.
        profile_name: String,
    },
    /// Update the autosave status chip.
    ShowStatus(AutosaveStatus),
}
