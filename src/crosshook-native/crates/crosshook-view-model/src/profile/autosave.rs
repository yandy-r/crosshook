//! Pure profile autosave state machine (`ProfileAutosave`).
//!
//! Ported from Classic React autosave hooks (`useHeroProfilesAutosave.ts`,
//! `useProfileLaunchAutosave.ts`, `useProfileLaunchAutosaveEffects.ts`,
//! `useLaunchEnvironmentAutosave.ts`, `useHeroLaunchHooksAutosave.ts`,
//! `useHeroTrainerAutosave.ts` and `useAutoSaveChip.ts`).
//!
//! All state transitions happen synchronously inside [`ProfileAutosave::apply`]
//! (and [`ViewModel::reduce`]) against an injected [`AutosaveInstant`] clock,
//! returning plain [`AutosaveEffect`] values for the UI bridge to execute.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ViewModel;

#[cfg(feature = "app")]
mod app;
mod dispatch;
pub mod effects;
pub mod events;
mod helpers;
pub mod policy;
pub mod state;
mod transitions;

#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;

#[cfg(feature = "app")]
pub use app::external_changes_from_app_event;
pub use effects::{AutosaveEffect, ConflictDetails, SaveRequest};
pub use events::{
    AutosaveEvent, AutosaveIntent, ConflictResolution, ExternalChange, ProfileChangeKind,
};
pub use policy::{
    combine_statuses, AutosavePolicy, AutosaveStatus, AutosaveTone, DEFAULT_ENVIRONMENT_DEBOUNCE,
    DEFAULT_MAX_WAIT, DEFAULT_RETRY_BACKOFF_MS, DEFAULT_SAVED_STATUS_HOLD,
    DEFAULT_SECTION_DEBOUNCE,
};
pub use state::{
    AutosaveInstant, AutosaveScope, AutosaveSection, AutosaveState, ReloadId, RevisionHash, SaveId,
    SaveTarget, SectionSaveKind,
};

pub(super) const MAX_PENDING_SELF_WRITES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirtyEntry {
    pub(super) since: AutosaveInstant,
    pub(super) deadline: AutosaveInstant,
    pub(super) revision: RevisionHash,
    pub(super) edit_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InFlightSave {
    pub(super) id: SaveId,
    pub(super) target: SaveTarget,
    pub(super) entries: BTreeMap<AutosaveSection, DirtyEntry>,
    pub(super) revision: RevisionHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingSelfWrite {
    pub(super) id: SaveId,
    pub(super) revision: RevisionHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingReload {
    pub(super) id: ReloadId,
    pub(super) profile_name: String,
    pub(super) reason: ProfileChangeKind,
    pub(super) save_seq_at_request: u64,
}

/// Pure, deterministic autosave reducer for one profile editor instance.
///
/// Not `Clone`: each instance owns a process-unique `mount` id so stale replies
/// from an earlier editor session can never match a newer one.
#[derive(Debug)]
pub struct ProfileAutosave {
    pub(super) mount: u64,
    pub(super) last_save_seq: u64,
    pub(super) last_reload_seq: u64,
    pub(super) scope: AutosaveScope,
    pub(super) policy: AutosavePolicy,
    pub(super) state: AutosaveState,
    pub(super) status: AutosaveStatus,
    pub(super) base_revision: Option<RevisionHash>,
    pub(super) latest_draft_revision: Option<RevisionHash>,
    pub(super) section_saved_revs: BTreeMap<AutosaveSection, RevisionHash>,
    pub(super) dirty_sections: BTreeMap<AutosaveSection, DirtyEntry>,
    pub(super) in_flight: Option<InFlightSave>,
    pub(super) pending_self_writes: VecDeque<PendingSelfWrite>,
    pub(super) scheduled_tick: Option<AutosaveInstant>,
    pub(super) retry_attempt: u32,
    pub(super) next_edit_seq: u64,
    pub(super) conflict_reload_name: Option<String>,
    pub(super) pending_resolution: Option<(ConflictResolution, AutosaveInstant)>,
    pub(super) failed_sections: BTreeSet<AutosaveSection>,
    pub(super) disk_base_matches: bool,
    pub(super) pending_flush: bool,
    pub(super) pending_reload: Option<PendingReload>,
}

impl ProfileAutosave {
    /// Creates an idle autosave state machine with [`AutosavePolicy::default`].
    #[must_use]
    pub fn new(scope: AutosaveScope, base_revision: Option<RevisionHash>) -> Self {
        Self::with_policy(scope, base_revision, AutosavePolicy::default())
    }

    /// Creates an idle autosave state machine with an explicit `policy`.
    #[must_use]
    pub fn with_policy(
        scope: AutosaveScope,
        base_revision: Option<RevisionHash>,
        policy: AutosavePolicy,
    ) -> Self {
        static NEXT_MOUNT: AtomicU64 = AtomicU64::new(1);
        Self {
            mount: NEXT_MOUNT.fetch_add(1, Ordering::Relaxed),
            last_save_seq: 0,
            last_reload_seq: 0,
            scope,
            policy,
            state: AutosaveState::Idle,
            status: AutosaveStatus::idle(),
            base_revision: base_revision.clone(),
            latest_draft_revision: base_revision.clone(),
            section_saved_revs: BTreeMap::new(),
            dirty_sections: BTreeMap::new(),
            in_flight: None,
            pending_self_writes: VecDeque::new(),
            scheduled_tick: None,
            retry_attempt: 0,
            next_edit_seq: 0,
            conflict_reload_name: None,
            pending_resolution: None,
            failed_sections: BTreeSet::new(),
            disk_base_matches: base_revision.is_some(),
            pending_flush: false,
            pending_reload: None,
        }
    }

    /// Applies one domain event at monotonic timestamp `now` and returns the
    /// ordered effects the bridge must run.
    #[must_use]
    pub fn apply(&mut self, event: AutosaveEvent, now: AutosaveInstant) -> Vec<AutosaveEffect> {
        let mut effects = Vec::new();
        match event {
            AutosaveEvent::Loaded {
                scope,
                revision,
                reload_id,
            } => {
                self.on_loaded(scope, revision, reload_id, &mut effects);
            }
            AutosaveEvent::ScopeChanged(scope) => {
                self.on_scope_changed(scope, now, &mut effects);
            }
            AutosaveEvent::Edit { section, revision } => {
                self.on_edit(section, revision, now, &mut effects);
            }
            AutosaveEvent::Tick => self.on_tick(now, &mut effects),
            AutosaveEvent::Flush => self.on_flush(&mut effects),
            AutosaveEvent::RetryNow => self.on_retry_now(&mut effects),
            AutosaveEvent::SaveSucceeded { id, committed_rev } => {
                self.on_save_succeeded(id, committed_rev, now, &mut effects);
            }
            AutosaveEvent::SaveFailed { id, error } => {
                self.on_save_failed(id, error, now, &mut effects);
            }
            AutosaveEvent::ExternalChange(change) => {
                self.on_external_change(change, &mut effects);
            }
            AutosaveEvent::ResolveConflict(resolution) => {
                self.on_resolve_conflict(resolution, now, &mut effects);
            }
        }
        effects
    }

    /// Current autosave state.
    #[must_use]
    pub const fn state(&self) -> &AutosaveState {
        &self.state
    }

    /// Current editor scope.
    #[must_use]
    pub const fn scope(&self) -> &AutosaveScope {
        &self.scope
    }

    /// Configured timing and retry policy.
    #[must_use]
    pub const fn policy(&self) -> &AutosavePolicy {
        &self.policy
    }

    /// Overall autosave status badge.
    #[must_use]
    pub const fn status(&self) -> &AutosaveStatus {
        &self.status
    }

    /// Revision hash of the last confirmed on-disk profile, if known.
    #[must_use]
    pub const fn base_revision(&self) -> Option<&RevisionHash> {
        self.base_revision.as_ref()
    }

    /// Revision hash of the latest local full-profile draft, if known.
    #[must_use]
    pub const fn latest_draft_revision(&self) -> Option<&RevisionHash> {
        self.latest_draft_revision.as_ref()
    }

    /// Whether any local edits have not yet been persisted to disk.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.dirty_sections.is_empty()
            || self.in_flight.is_some()
            || matches!(self.state, AutosaveState::Conflict { .. })
    }

    /// All sections that currently hold unpersisted or in-flight local edits.
    #[must_use]
    pub fn dirty_sections(&self) -> BTreeSet<AutosaveSection> {
        let mut set: BTreeSet<AutosaveSection> = self.dirty_sections.keys().copied().collect();
        if let Some(in_flight) = &self.in_flight {
            set.extend(in_flight.entries.keys().copied());
        }
        if let AutosaveState::Conflict { sections, .. } = &self.state {
            set.extend(sections.iter().copied());
        }
        set
    }

    /// Next scheduled wake-up timestamp, or `None` when idle / untimed.
    #[must_use]
    pub const fn scheduled_tick(&self) -> Option<AutosaveInstant> {
        self.scheduled_tick
    }

    /// Per-section status for UI badges and [`combine_statuses`].
    #[must_use]
    pub fn section_status(&self, section: AutosaveSection) -> AutosaveStatus {
        if !self.scope.method_supports_section(section) {
            return AutosaveStatus::warning("Unavailable for current method", None);
        }
        if !self.scope.profile_exists {
            if self.dirty_sections.contains_key(&section) || section.requires_proton_or_steam() {
                return AutosaveStatus::warning("Save profile first", None);
            }
            return AutosaveStatus::idle();
        }
        if !self.scope.can_autosave() {
            return AutosaveStatus::idle();
        }
        if matches!(self.state, AutosaveState::Conflict { .. }) {
            return AutosaveStatus::conflict();
        }
        if let Some(in_flight) = &self.in_flight {
            if in_flight.entries.contains_key(&section)
                || self.dirty_sections.contains_key(&section)
            {
                return AutosaveStatus::saving();
            }
        }
        if self.dirty_sections.contains_key(&section) {
            if let AutosaveState::Error { error, .. } = &self.state {
                if self.failed_sections.contains(&section) {
                    return AutosaveStatus::error("Failed to save", Some(error.message.clone()));
                }
            }
            return AutosaveStatus::saving();
        }
        if matches!(self.state, AutosaveState::Saved { .. }) {
            return AutosaveStatus::saved();
        }
        AutosaveStatus::idle()
    }

    #[cfg(test)]
    pub(super) fn has_queued_dirty_section(&self, section: AutosaveSection) -> bool {
        self.dirty_sections.contains_key(&section)
    }
}

impl ViewModel for ProfileAutosave {
    type Intent = AutosaveIntent;
    type Effect = AutosaveEffect;

    fn reduce(&mut self, intent: AutosaveIntent) -> Vec<AutosaveEffect> {
        self.apply(intent.event, intent.now)
    }
}
