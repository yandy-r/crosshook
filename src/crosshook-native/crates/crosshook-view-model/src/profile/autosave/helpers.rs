//! Internal state, deadline, status and self-write helpers for
//! [`super::ProfileAutosave`].

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use super::effects::AutosaveEffect;
use super::policy::{AutosavePolicy, AutosaveStatus};
use super::state::{AutosaveInstant, AutosaveSection, AutosaveState, RevisionHash, SaveTarget};
use super::{DirtyEntry, ProfileAutosave, MAX_PENDING_SELF_WRITES};

impl ProfileAutosave {
    pub(super) fn reset_working_state(&mut self) {
        self.dirty_sections.clear();
        self.in_flight = None;
        self.pending_self_writes.clear();
        self.section_saved_revs.clear();
        self.scheduled_tick = None;
        self.retry_attempt = 0;
        self.conflict_reload_name = None;
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
        self.in_flight
            .as_ref()
            .is_some_and(|f| f.entries.contains_key(&section))
    }

    pub(super) fn matches_saved_revision(
        &self,
        section: AutosaveSection,
        revision: &RevisionHash,
    ) -> bool {
        if self.section_saved_revs.get(&section) == Some(revision) {
            return true;
        }
        let other_dirty = self.dirty_sections.keys().any(|k| *k != section);
        !other_dirty && self.base_revision.as_ref() == Some(revision)
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

    pub(super) fn record_pending_self_write(&mut self, rev: RevisionHash) {
        if self.pending_self_writes.len() >= MAX_PENDING_SELF_WRITES {
            self.pending_self_writes.pop_front();
        }
        self.pending_self_writes.push_back(rev);
    }

    pub(super) fn remove_pending_self_write(&mut self, rev: &RevisionHash) -> bool {
        if let Some(pos) = self.pending_self_writes.iter().position(|r| r == rev) {
            self.pending_self_writes.remove(pos);
            true
        } else {
            false
        }
    }

    pub(super) fn update_pending_self_write(&mut self, old: &RevisionHash, new: &RevisionHash) {
        if let Some(slot) = self.pending_self_writes.iter_mut().find(|r| *r == old) {
            *slot = new.clone();
        }
    }

    pub(super) fn emit_status(&mut self, next: AutosaveStatus, effects: &mut Vec<AutosaveEffect>) {
        if self.status != next {
            self.status = next.clone();
            effects.push(AutosaveEffect::ShowStatus(next));
        }
    }
}
