//! External change / conflict resolution and save dispatch for
//! [`super::ProfileAutosave`].

use std::collections::{BTreeMap, BTreeSet};

use super::effects::{AutosaveEffect, ConflictDetails, SaveRequest};
use super::events::{ConflictResolution, ExternalChange, ProfileChangeKind};
use super::policy::AutosaveStatus;
use super::state::{
    AutosaveInstant, AutosaveSection, AutosaveState, RevisionHash, SaveId, SaveTarget,
};
use super::{DirtyEntry, InFlightSave, ProfileAutosave};

impl ProfileAutosave {
    pub(super) fn on_external_change(
        &mut self,
        change: ExternalChange,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if change.reason == ProfileChangeKind::FavoriteToggled {
            return;
        }
        let active_name = self.scope.profile_name().to_owned();
        if active_name.is_empty() {
            return;
        }
        let renamed_from_active = matches!(
            &change.reason,
            ProfileChangeKind::Renamed { from } if from.trim() == active_name
        );
        if let Some(target_name) = change.profile_name.as_deref() {
            if target_name.trim() != active_name && !renamed_from_active {
                return;
            }
        }

        if matches!(
            change.reason,
            ProfileChangeKind::Saved | ProfileChangeKind::PresetApplied
        ) {
            if let Some(rev) = &change.revision {
                if self.remove_pending_self_write(rev) {
                    return;
                }
                if matches!(
                    self.state,
                    AutosaveState::Idle | AutosaveState::Saved { .. }
                ) && self.base_revision.as_ref() == Some(rev)
                {
                    return;
                }
            }
        }

        let reload_name = if renamed_from_active {
            change
                .profile_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&active_name)
                .to_owned()
        } else {
            active_name
        };

        self.scheduled_tick = None;
        if change.reason == ProfileChangeKind::Resync {
            self.pending_self_writes.clear();
        }

        match &self.state {
            AutosaveState::Idle | AutosaveState::Saved { .. } if self.dirty_sections.is_empty() => {
                self.conflict_reload_name = None;
                if let Some(rev) = change.revision {
                    self.base_revision = Some(rev.clone());
                    self.latest_draft_revision = Some(rev);
                }
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
                effects.push(AutosaveEffect::Reload {
                    profile_name: reload_name,
                });
            }
            AutosaveState::Conflict {
                local, sections, ..
            } => {
                if renamed_from_active {
                    self.conflict_reload_name = Some(reload_name.clone());
                }
                let details = ConflictDetails {
                    profile_name: reload_name,
                    local: local.clone(),
                    remote: change.revision.clone(),
                    sections: sections.clone(),
                    reason: change.reason,
                };
                self.state = AutosaveState::Conflict {
                    local: details.local.clone(),
                    remote: details.remote.clone(),
                    sections: details.sections.clone(),
                };
                effects.push(AutosaveEffect::ShowConflict(details));
            }
            _ => {
                let sections = self.dirty_sections();
                let local = self.conflict_local_revision();
                self.in_flight = None;
                self.conflict_reload_name = renamed_from_active.then(|| reload_name.clone());
                let details = ConflictDetails {
                    profile_name: reload_name,
                    local: local.clone(),
                    remote: change.revision.clone(),
                    sections: sections.clone(),
                    reason: change.reason,
                };
                self.state = AutosaveState::Conflict {
                    local,
                    remote: change.revision,
                    sections,
                };
                self.emit_status(AutosaveStatus::conflict(), effects);
                effects.push(AutosaveEffect::ShowConflict(details));
            }
        }
    }

    pub(super) fn on_resolve_conflict(
        &mut self,
        resolution: ConflictResolution,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let AutosaveState::Conflict {
            local,
            remote,
            sections,
        } = self.state.clone()
        else {
            return;
        };

        let profile_name = self.scope.profile_name().to_owned();
        if profile_name.is_empty() {
            return;
        }

        match resolution {
            ConflictResolution::KeepMine => {
                self.retry_attempt = 0;
                self.scheduled_tick = None;
                self.conflict_reload_name = None;
                let mut entries = BTreeMap::new();
                for sec in &sections {
                    let entry = self
                        .dirty_sections
                        .remove(sec)
                        .unwrap_or_else(|| DirtyEntry {
                            since: now,
                            deadline: now,
                            revision: local.clone(),
                            edit_seq: self.next_edit_seq,
                        });
                    entries.insert(*sec, entry);
                }
                self.dirty_sections.clear();
                self.start_save(SaveTarget::FullProfile, entries, local, false, effects);
            }
            ConflictResolution::ReloadTheirs => {
                let target_name = self.conflict_reload_name.take().unwrap_or(profile_name);
                self.reset_working_state();
                self.base_revision = remote.clone();
                self.latest_draft_revision = remote;
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
                effects.push(AutosaveEffect::Reload {
                    profile_name: target_name,
                });
            }
        }
    }

    pub(super) fn dispatch_due(
        &mut self,
        due: &BTreeSet<AutosaveSection>,
        flush_remaining: bool,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let eligible = self.eligible_dirty_sections();
        let has_due_full = due
            .iter()
            .any(|sec| sec.save_target() == SaveTarget::FullProfile);
        if has_due_full {
            self.dispatch_full_profile(&eligible, effects);
            return;
        }

        if let Some(first_due) = due.iter().copied().next() {
            self.dispatch_single_section(first_due, flush_remaining, effects);
        }
    }

    pub(super) fn dispatch_all_eligible(
        &mut self,
        flush_remaining: bool,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let eligible = self.eligible_dirty_sections();
        if eligible.is_empty() {
            self.scheduled_tick = None;
            if self.dirty_sections.is_empty() {
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
            } else if let AutosaveState::Error { retry_at, .. } = &mut self.state {
                *retry_at = None;
            }
            return;
        }
        if eligible
            .iter()
            .any(|sec| sec.save_target() == SaveTarget::FullProfile)
        {
            self.dispatch_full_profile(&eligible, effects);
            return;
        }

        if let Some(first) = eligible.iter().copied().next() {
            self.dispatch_single_section(first, flush_remaining, effects);
        }
    }

    pub(super) fn dispatch_single_section(
        &mut self,
        section: AutosaveSection,
        flush_remaining: bool,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let Some(entry) = self.dirty_sections.remove(&section) else {
            return;
        };
        let flush_queued = flush_remaining && !self.eligible_dirty_sections().is_empty();
        let revision = entry.revision.clone();
        let mut entries = BTreeMap::new();
        entries.insert(section, entry);
        self.start_save(
            section.save_target(),
            entries,
            revision,
            flush_queued,
            effects,
        );
    }

    pub(super) fn dispatch_full_profile(
        &mut self,
        eligible: &BTreeSet<AutosaveSection>,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let mut entries = BTreeMap::new();
        for sec in eligible {
            if let Some(entry) = self.dirty_sections.remove(sec) {
                entries.insert(*sec, entry);
            }
        }
        let revision = entries
            .iter()
            .filter(|(sec, _)| sec.save_target() == SaveTarget::FullProfile)
            .max_by_key(|(_, entry)| entry.edit_seq)
            .map(|(_, entry)| entry.revision.clone())
            .or_else(|| self.latest_draft_revision.clone())
            .or_else(|| {
                entries
                    .values()
                    .max_by_key(|entry| entry.edit_seq)
                    .map(|entry| entry.revision.clone())
            })
            .unwrap_or_else(|| RevisionHash::new(""));
        self.start_save(SaveTarget::FullProfile, entries, revision, false, effects);
    }

    pub(super) fn start_save(
        &mut self,
        target: SaveTarget,
        entries: BTreeMap<AutosaveSection, DirtyEntry>,
        revision: RevisionHash,
        flush_queued: bool,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        self.last_save_seq = self.last_save_seq.wrapping_add(1);
        let id = SaveId {
            mount: self.mount,
            seq: self.last_save_seq,
        };
        let sections: BTreeSet<AutosaveSection> = entries.keys().copied().collect();
        self.scheduled_tick = None;
        self.record_pending_self_write(revision.clone());
        self.in_flight = Some(InFlightSave {
            id,
            target,
            entries,
            revision: revision.clone(),
            flush_queued,
        });
        self.state = AutosaveState::Saving {
            id,
            target,
            sections: sections.clone(),
            rev: revision.clone(),
        };
        self.emit_status(AutosaveStatus::saving(), effects);
        effects.push(AutosaveEffect::Save(SaveRequest {
            id,
            profile_name: self.scope.profile_name().to_owned(),
            target,
            sections,
            revision,
        }));
    }
}
