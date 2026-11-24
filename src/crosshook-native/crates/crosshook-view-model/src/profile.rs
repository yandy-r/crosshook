//! Profile view-models, starting with [`autosave::ProfileAutosave`].
//!
//! All state here is runtime-only: nothing reads or writes `settings.toml` or
//! `metadata.db`. Persistence stays in `crosshook-app`'s `ProfileWriter`,
//! driven by the effects [`autosave::ProfileAutosave`] returns.

pub mod autosave;

#[cfg(feature = "app")]
pub use autosave::external_changes_from_app_event;
pub use autosave::{
    combine_statuses, AutosaveEffect, AutosaveEvent, AutosaveInstant, AutosaveIntent,
    AutosavePolicy, AutosaveScope, AutosaveSection, AutosaveState, AutosaveStatus, AutosaveTone,
    ConflictDetails, ConflictResolution, ExternalChange, ProfileAutosave, ProfileChangeKind,
    RevisionHash, SaveId, SaveRequest, SaveTarget, SectionSaveKind,
};
