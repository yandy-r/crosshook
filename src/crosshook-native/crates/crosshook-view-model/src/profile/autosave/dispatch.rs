//! External change / conflict resolution and save dispatch for
//! [`super::ProfileAutosave`].

use std::collections::{BTreeMap, BTreeSet};

use super::effects::{AutosaveEffect, ConflictDetails, SaveRequest};
use super::events::{ConflictResolution, ExternalChange, ProfileChangeKind};
use super::policy::AutosaveStatus;
use super::state::{
    AutosaveInstant, AutosaveSection, AutosaveState, ReloadId, RevisionHash, SaveId, SaveTarget,
};
use super::{DirtyEntry, InFlightSave, PendingReload, ProfileAutosave};

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
        let tracked_name = if matches!(self.state, AutosaveState::Conflict { .. }) {
            self.conflict_reload_name.as_deref()
        } else {
            self.pending_reload
                .as_ref()
                .map(|pending| pending.profile_name.as_str())
        }
        .map(str::to_owned);
        let renamed_from_active = matches!(
            &change.reason,
            ProfileChangeKind::Renamed { from }
                if from.trim() == active_name
                    || tracked_name.as_deref() == Some(from.trim())
        );
        if let Some(target_name) = change.profile_name.as_deref() {
            let trimmed_target = target_name.trim();
            if trimmed_target != active_name
                && tracked_name.as_deref() != Some(trimmed_target)
                && !renamed_from_active
            {
                return;
            }
        }

        if matches!(
            change.reason,
            ProfileChangeKind::Saved | ProfileChangeKind::PresetApplied
        ) {
            if let Some(origin_id) = change.origin_save_id {
                if self.remove_pending_self_write_by_id(origin_id) {
                    return;
                }
            } else if let Some(rev) = &change.revision {
                if self.remove_pending_self_write_by_rev(rev) {
                    return;
                }
            }
            if let Some(rev) = &change.revision {
                if matches!(
                    self.state,
                    AutosaveState::Idle | AutosaveState::Saved { .. }
                ) && self.disk_base_matches
                    && self.base_revision.as_ref() == Some(rev)
                {
                    return;
                }
            }
        }

        let default_name = tracked_name.as_deref().unwrap_or(&active_name);
        let reload_name = if renamed_from_active {
            change
                .profile_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(default_name)
                .to_owned()
        } else {
            default_name.to_owned()
        };

        self.scheduled_tick = None;
        if change.reason == ProfileChangeKind::Resync {
            self.pending_self_writes.clear();
        }

        match &self.state {
            AutosaveState::Idle | AutosaveState::Saved { .. } if self.dirty_sections.is_empty() => {
                let reload_reason = change.reason;
                self.conflict_reload_name = None;
                self.pending_resolution = None;
                self.pending_flush = false;
                self.failed_sections.clear();
                self.section_saved_revs.clear();
                let remote = change.revision.clone();
                self.disk_base_matches = remote.is_some();
                if let Some(rev) = change.revision {
                    self.base_revision = Some(rev.clone());
                    self.latest_draft_revision = Some(rev);
                }
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
                self.request_reload(reload_name, reload_reason, remote, effects);
            }
            AutosaveState::Conflict {
                local, sections, ..
            } => {
                self.pending_resolution = None;
                self.pending_flush = false;
                self.pending_reload = None;
                self.failed_sections.clear();
                self.section_saved_revs.clear();
                self.disk_base_matches = false;
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
                self.pending_resolution = None;
                self.pending_flush = false;
                self.pending_reload = None;
                self.failed_sections.clear();
                self.section_saved_revs.clear();
                self.disk_base_matches = false;
                self.conflict_reload_name =
                    (reload_name != active_name).then(|| reload_name.clone());
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

    pub(super) fn enter_conflict_with_target(
        &mut self,
        target_name: String,
        remote: Option<RevisionHash>,
        reason: ProfileChangeKind,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let sections = self.dirty_sections();
        let local = self.conflict_local_revision();
        let active_name = self.scope.profile_name().to_owned();
        let conflict_name = if target_name.is_empty() {
            active_name.clone()
        } else {
            target_name
        };
        self.scheduled_tick = None;
        self.pending_flush = false;
        self.pending_reload = None;
        self.pending_resolution = None;
        self.failed_sections.clear();
        self.section_saved_revs.clear();
        self.disk_base_matches = false;
        self.conflict_reload_name = (conflict_name != active_name).then(|| conflict_name.clone());
        let details = ConflictDetails {
            profile_name: conflict_name,
            local: local.clone(),
            remote: remote.clone(),
            sections: sections.clone(),
            reason,
        };
        self.state = AutosaveState::Conflict {
            local,
            remote,
            sections,
        };
        self.emit_status(AutosaveStatus::conflict(), effects);
        effects.push(AutosaveEffect::ShowConflict(details));
    }

    pub(super) fn on_resolve_conflict(
        &mut self,
        resolution: ConflictResolution,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if !matches!(self.state, AutosaveState::Conflict { .. })
            || self.scope.profile_name().is_empty()
        {
            return;
        }
        if self.in_flight.is_some() {
            self.pending_resolution = Some((resolution, now));
            return;
        }
        self.execute_conflict_resolution(resolution, now, effects);
    }

    pub(super) fn settle_in_flight_during_conflict(
        &mut self,
        in_flight: InFlightSave,
        succeeded_rev: Option<&RevisionHash>,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        match succeeded_rev {
            Some(committed_rev) => {
                if committed_rev != &in_flight.revision {
                    self.update_pending_self_write(in_flight.id, committed_rev);
                }
            }
            None => {
                self.remove_pending_self_write_by_id(in_flight.id);
            }
        }
        for (sec, entry) in in_flight.entries {
            self.dirty_sections.entry(sec).or_insert(entry);
        }
        if let Some((resolution, res_now)) = self.pending_resolution.take() {
            self.execute_conflict_resolution(resolution, res_now.max(now), effects);
        }
    }

    pub(super) fn execute_conflict_resolution(
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
                let target_name = self.conflict_reload_name.take().unwrap_or(profile_name);
                if target_name != self.scope.profile_name() {
                    let old_selected = self.scope.profile_name().to_owned();
                    self.scope.selected_profile = target_name.clone();
                    if self.scope.draft_name.trim() == old_selected {
                        self.scope.draft_name = target_name;
                    }
                }
                self.retry_attempt = 0;
                self.scheduled_tick = None;
                self.pending_resolution = None;
                self.pending_flush = false;
                self.pending_reload = None;
                self.failed_sections.clear();
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
                self.disk_base_matches = remote.is_some();
                self.base_revision = remote.clone();
                self.latest_draft_revision = remote.clone();
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
                self.request_reload(target_name, ProfileChangeKind::Saved, remote, effects);
            }
        }
    }

    pub(super) fn request_reload(
        &mut self,
        profile_name: String,
        reason: ProfileChangeKind,
        remote: Option<RevisionHash>,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        self.last_reload_seq = self.last_reload_seq.wrapping_add(1);
        let id = ReloadId {
            mount: self.mount,
            seq: self.last_reload_seq,
        };
        self.pending_reload = Some(PendingReload {
            id,
            profile_name: profile_name.clone(),
            reason,
            remote,
        });
        effects.push(AutosaveEffect::Reload { id, profile_name });
    }

    fn conflict_if_reload_pending(&mut self, effects: &mut Vec<AutosaveEffect>) -> bool {
        let Some(reload) = self.pending_reload.take() else {
            return false;
        };
        self.enter_conflict_with_target(reload.profile_name, reload.remote, reload.reason, effects);
        true
    }

    pub(super) fn dispatch_due(
        &mut self,
        due: &BTreeSet<AutosaveSection>,
        flush_remaining: bool,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if self.conflict_if_reload_pending(effects) {
            return;
        }
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
                self.failed_sections.clear();
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
            } else if let AutosaveState::Error { retry_at, .. } = &mut self.state {
                *retry_at = None;
            }
            return;
        }
        if self.conflict_if_reload_pending(effects) {
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
        self.pending_flush = flush_queued;
        self.failed_sections.clear();
        self.record_pending_self_write(id, revision.clone());
        self.in_flight = Some(InFlightSave {
            id,
            target,
            entries,
            revision: revision.clone(),
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
