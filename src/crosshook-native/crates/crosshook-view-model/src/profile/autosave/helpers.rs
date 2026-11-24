//! Internal state, deadline, status and self-write helpers for
//! [`super::ProfileAutosave`].

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use super::effects::AutosaveEffect;
use super::events::ConflictResolution;
use super::policy::{AutosavePolicy, AutosaveStatus};
use super::state::{
    AutosaveInstant, AutosaveSection, AutosaveState, RevisionHash, SaveId, SaveTarget,
};
use super::{DirtyEntry, PendingSelfWrite, ProfileAutosave, MAX_PENDING_SELF_WRITES};

impl ProfileAutosave {
    pub(super) fn reset_working_state(&mut self) {
        self.dirty_sections.clear();
        self.in_flight = None;
        self.pending_self_writes.clear();
        self.section_saved_revs.clear();
        self.scheduled_tick = None;
        self.retry_attempt = 0;
        self.conflict_reload_name = None;
        self.pending_resolution = None;
        self.failed_sections.clear();
        self.pending_flush = false;
        self.pending_reload = None;
    }

    pub(super) fn eligible_dirty_sections(&self) -> BTreeSet<AutosaveSection> {
        self.dirty_sections
            .keys()
            .copied()
            .filter(|sec| self.scope.can_autosave_section(*sec))
            .collect()
    }

    pub(super) fn earliest_eligible_deadline(&self) -> Option<AutosaveInstant> {
        self.dirty_sections
            .iter()
            .filter(|(sec, _)| self.scope.can_autosave_section(**sec))
            .map(|(_, entry)| entry.deadline)
            .min()
    }

    pub(super) fn sync_schedule_tick(&mut self, effects: &mut Vec<AutosaveEffect>) {
        let next = self.earliest_eligible_deadline();
        if next != self.scheduled_tick {
            self.scheduled_tick = next;
            if let Some(at) = next {
                effects.push(AutosaveEffect::ScheduleTick(at));
            }
        }
    }

    pub(super) fn set_dirty_state(&mut self) {
        self.state = self.dirty_state_from_entries();
    }

    pub(super) fn dirty_state_from_entries(&self) -> AutosaveState {
        let since = self
            .dirty_sections
            .values()
            .map(|e| e.since)
            .min()
            .unwrap_or_else(|| AutosaveInstant::from_millis(0));
        let sections = self.dirty_sections.keys().copied().collect();
        AutosaveState::Dirty { since, sections }
    }

    pub(super) fn compute_deadline_for(
        policy: &AutosavePolicy,
        now: AutosaveInstant,
        since: AutosaveInstant,
        debounce: Duration,
    ) -> AutosaveInstant {
        let debounced = now.saturating_add(debounce);
        match policy.max_wait {
            Some(max_wait) => debounced.min(since.saturating_add(max_wait)).max(now),
            None => debounced,
        }
    }

    pub(super) fn handle_method_gated_edit(
        &mut self,
        section: AutosaveSection,
        revision: &RevisionHash,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if self.dirty_sections.contains_key(&section) {
            if !matches!(self.state, AutosaveState::Conflict { .. })
                && !self.in_flight_covers(section)
                && self.matches_saved_revision(section, revision)
            {
                self.dirty_sections.remove(&section);
                self.failed_sections.remove(&section);
                self.finish_undo_section(now, effects);
                return;
            }

            self.next_edit_seq = self.next_edit_seq.wrapping_add(1);
            if let Some(entry) = self.dirty_sections.get_mut(&section) {
                entry.revision = revision.clone();
                entry.edit_seq = self.next_edit_seq;
            }
            if matches!(self.state, AutosaveState::Conflict { .. }) {
                self.record_conflict_edit(section, revision.clone());
            }
            return;
        }

        if let Some(in_flight_since) = self
            .in_flight
            .as_ref()
            .and_then(|f| f.entries.get(&section))
            .map(|e| e.since)
        {
            self.next_edit_seq = self.next_edit_seq.wrapping_add(1);
            let deadline = Self::compute_deadline_for(
                &self.policy,
                now,
                in_flight_since,
                self.policy.debounce_for(section),
            );
            self.dirty_sections.insert(
                section,
                DirtyEntry {
                    since: in_flight_since,
                    deadline,
                    revision: revision.clone(),
                    edit_seq: self.next_edit_seq,
                },
            );
            if matches!(self.state, AutosaveState::Conflict { .. }) {
                self.record_conflict_edit(section, revision.clone());
            }
            return;
        }

        if self.dirty_sections.is_empty()
            && self.in_flight.is_none()
            && !matches!(
                self.state,
                AutosaveState::Error { .. } | AutosaveState::Conflict { .. }
            )
        {
            if matches!(self.state, AutosaveState::Saved { .. }) {
                self.state = AutosaveState::Idle;
                self.scheduled_tick = None;
            }
            self.emit_status(
                AutosaveStatus::warning("Unavailable for current method", None),
                effects,
            );
        }
    }

    pub(super) fn finish_undo_section(
        &mut self,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if self.in_flight.is_none() && self.dirty_sections.is_empty() {
            self.retry_attempt = 0;
            self.scheduled_tick = None;
            self.pending_flush = false;
            self.failed_sections.clear();
            self.state = AutosaveState::Idle;
            self.emit_status(AutosaveStatus::idle(), effects);
        } else if matches!(self.state, AutosaveState::Dirty { .. }) {
            self.set_dirty_state();
            if self.scope.can_autosave() && self.earliest_eligible_deadline().is_some() {
                self.sync_schedule_tick(effects);
            } else {
                self.scheduled_tick = None;
                let status = self.dirty_without_eligible_status();
                self.emit_status(status, effects);
            }
        } else if matches!(self.state, AutosaveState::Error { .. }) {
            if self.failed_sections.is_empty() {
                self.retry_attempt = 0;
                for entry in self.dirty_sections.values_mut() {
                    entry.deadline = entry.deadline.max(now);
                }
                self.set_dirty_state();
                if self.scope.can_autosave() && self.earliest_eligible_deadline().is_some() {
                    self.emit_status(AutosaveStatus::saving(), effects);
                    self.sync_schedule_tick(effects);
                } else {
                    self.scheduled_tick = None;
                    let status = self.dirty_without_eligible_status();
                    self.emit_status(status, effects);
                }
            } else if self.eligible_dirty_sections().is_empty() {
                if let AutosaveState::Error { retry_at, .. } = &mut self.state {
                    *retry_at = None;
                }
                self.scheduled_tick = None;
            }
        }
    }

    pub(super) fn record_conflict_edit(
        &mut self,
        section: AutosaveSection,
        revision: RevisionHash,
    ) {
        if let AutosaveState::Conflict {
            local, sections, ..
        } = &mut self.state
        {
            if section.save_target() == SaveTarget::FullProfile
                || !sections
                    .iter()
                    .any(|s| s.save_target() == SaveTarget::FullProfile)
            {
                *local = revision;
            }
            sections.insert(section);
            if matches!(
                self.pending_resolution,
                Some((ConflictResolution::ReloadTheirs, _))
            ) {
                self.pending_resolution = None;
            }
        }
    }

    pub(super) fn conflict_local_revision(&self) -> RevisionHash {
        self.dirty_sections
            .iter()
            .filter(|(sec, _)| sec.save_target() == SaveTarget::FullProfile)
            .max_by_key(|(_, entry)| entry.edit_seq)
            .map(|(_, entry)| entry.revision.clone())
            .or_else(|| {
                if self.latest_draft_revision != self.base_revision {
                    self.latest_draft_revision.clone()
                } else {
                    None
                }
            })
            .or_else(|| {
                self.dirty_sections
                    .values()
                    .max_by_key(|entry| entry.edit_seq)
                    .map(|entry| entry.revision.clone())
            })
            .or_else(|| self.in_flight.as_ref().map(|f| f.revision.clone()))
            .or_else(|| self.latest_draft_revision.clone())
            .or_else(|| self.base_revision.clone())
            .unwrap_or_else(|| RevisionHash::new(""))
    }

    pub(super) fn inactive_status(&self) -> AutosaveStatus {
        if !self.scope.profile_exists && !self.dirty_sections.is_empty() {
            AutosaveStatus::warning("Save profile first", None)
        } else {
            AutosaveStatus::idle()
        }
    }

    pub(super) fn dirty_without_eligible_status(&self) -> AutosaveStatus {
        if !self.scope.profile_exists {
            AutosaveStatus::warning("Save profile first", None)
        } else if !self.scope.can_autosave() {
            AutosaveStatus::idle()
        } else {
            AutosaveStatus::warning("Unavailable for current method", None)
        }
    }

    pub(super) fn in_flight_covers(&self, section: AutosaveSection) -> bool {
        self.in_flight.as_ref().is_some_and(|f| {
            f.target == SaveTarget::FullProfile || f.entries.contains_key(&section)
        })
    }

    pub(super) fn matches_saved_revision(
        &self,
        section: AutosaveSection,
        revision: &RevisionHash,
    ) -> bool {
        if section.save_target() != SaveTarget::FullProfile
            && self.section_saved_revs.get(&section) == Some(revision)
        {
            return true;
        }
        let other_dirty = self.dirty_sections.keys().any(|k| *k != section)
            || self
                .in_flight
                .as_ref()
                .is_some_and(|f| f.entries.keys().any(|k| *k != section));
        !other_dirty && self.disk_base_matches && self.base_revision.as_ref() == Some(revision)
    }

    pub(super) fn prune_clean_dirty_sections(
        &mut self,
        target: SaveTarget,
        in_flight_entries: &BTreeMap<AutosaveSection, DirtyEntry>,
        committed_rev: &RevisionHash,
    ) {
        let clean: Vec<AutosaveSection> = self
            .dirty_sections
            .iter()
            .filter(|(sec, entry)| match target {
                SaveTarget::FullProfile => {
                    (sec.save_target() == SaveTarget::FullProfile
                        && &entry.revision == committed_rev)
                        || (in_flight_entries.contains_key(sec)
                            && self.section_saved_revs.get(*sec) == Some(&entry.revision))
                }
                SaveTarget::Section(_) => {
                    in_flight_entries.contains_key(sec)
                        && self.section_saved_revs.get(*sec) == Some(&entry.revision)
                }
            })
            .map(|(sec, _)| *sec)
            .collect();
        for sec in clean {
            self.dirty_sections.remove(&sec);
        }
    }

    pub(super) fn record_pending_self_write(&mut self, id: SaveId, revision: RevisionHash) {
        if self.pending_self_writes.len() >= MAX_PENDING_SELF_WRITES {
            self.pending_self_writes.pop_front();
        }
        self.pending_self_writes
            .push_back(PendingSelfWrite { id, revision });
    }

    pub(super) fn remove_pending_self_write_by_id(&mut self, id: SaveId) -> bool {
        if let Some(pos) = self.pending_self_writes.iter().position(|w| w.id == id) {
            self.pending_self_writes.remove(pos);
            true
        } else {
            false
        }
    }

    pub(super) fn remove_pending_self_write_by_rev(&mut self, rev: &RevisionHash) -> bool {
        if let Some(pos) = self
            .pending_self_writes
            .iter()
            .position(|w| &w.revision == rev)
        {
            self.pending_self_writes.remove(pos);
            true
        } else {
            false
        }
    }

    pub(super) fn update_pending_self_write(&mut self, id: SaveId, new: &RevisionHash) {
        if let Some(slot) = self.pending_self_writes.iter_mut().find(|w| w.id == id) {
            slot.revision = new.clone();
        }
    }

    pub(super) fn handle_cancelled_save(
        &mut self,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        self.failed_sections.clear();
        let eligible = self.eligible_dirty_sections();
        if eligible.is_empty() || !self.scope.can_autosave() {
            self.scheduled_tick = None;
            let status = if self.dirty_sections.is_empty() {
                self.pending_flush = false;
                self.state = AutosaveState::Idle;
                AutosaveStatus::idle()
            } else {
                self.set_dirty_state();
                self.dirty_without_eligible_status()
            };
            self.emit_status(status, effects);
            return;
        }
        for sec in eligible {
            if let Some(entry) = self.dirty_sections.get_mut(&sec) {
                let min_ready = now.saturating_add(self.policy.debounce_for(sec));
                entry.deadline = entry.deadline.max(min_ready);
            }
        }
        if self.pending_flush {
            self.pending_flush = false;
            self.dispatch_all_eligible(false, effects);
            return;
        }
        self.set_dirty_state();
        self.emit_status(AutosaveStatus::saving(), effects);
        self.sync_schedule_tick(effects);
    }

    pub(super) fn emit_status(&mut self, next: AutosaveStatus, effects: &mut Vec<AutosaveEffect>) {
        if self.status != next {
            self.status = next.clone();
            effects.push(AutosaveEffect::ShowStatus(next));
        }
    }
}

pub(super) fn is_retryable_error(kind: &crate::ServiceErrorKind) -> bool {
    matches!(
        kind,
        crate::ServiceErrorKind::Io
            | crate::ServiceErrorKind::Unavailable
            | crate::ServiceErrorKind::Internal
    )
}
