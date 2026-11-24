//! Core state transitions for [`super::ProfileAutosave`] (load, scope, edit,
//! tick, flush, save completion).

use std::collections::BTreeSet;

use crate::{ServiceError, ServiceErrorKind};

use super::effects::AutosaveEffect;
use super::policy::AutosaveStatus;
use super::state::{
    AutosaveInstant, AutosaveScope, AutosaveSection, AutosaveState, RevisionHash, SaveId,
    SaveTarget,
};
use super::{DirtyEntry, ProfileAutosave};

impl ProfileAutosave {
    pub(super) fn on_loaded(
        &mut self,
        scope: AutosaveScope,
        revision: RevisionHash,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        self.reset_working_state();
        self.scope = scope;
        self.base_revision = Some(revision.clone());
        self.latest_draft_revision = Some(revision);
        self.state = AutosaveState::Idle;
        self.emit_status(AutosaveStatus::idle(), effects);
    }

    pub(super) fn on_scope_changed(
        &mut self,
        next_scope: AutosaveScope,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if next_scope.profile_name() != self.scope.profile_name() {
            self.reset_working_state();
            self.scope = next_scope;
            self.base_revision = None;
            self.latest_draft_revision = None;
            self.state = AutosaveState::Idle;
            self.emit_status(AutosaveStatus::idle(), effects);
            return;
        }

        let prev_scope = std::mem::replace(&mut self.scope, next_scope);
        if !self.scope.can_autosave() {
            self.scheduled_tick = None;
            match &mut self.state {
                AutosaveState::Error { retry_at, .. } => {
                    *retry_at = None;
                }
                AutosaveState::Saving { .. } | AutosaveState::Conflict { .. } => {}
                AutosaveState::Saved { .. } => {
                    self.state = AutosaveState::Idle;
                    self.emit_status(self.inactive_status(), effects);
                }
                AutosaveState::Idle | AutosaveState::Dirty { .. } => {
                    self.emit_status(self.inactive_status(), effects);
                }
            }
            return;
        }

        let has_eligible = !self.eligible_dirty_sections().is_empty();
        match &mut self.state {
            AutosaveState::Dirty { .. } => {
                for (&sec, entry) in &mut self.dirty_sections {
                    if self.scope.can_autosave_section(sec) && !prev_scope.can_autosave_section(sec)
                    {
                        entry.deadline = entry.deadline.max(now);
                    }
                }
                if self.earliest_eligible_deadline().is_some() {
                    self.emit_status(AutosaveStatus::saving(), effects);
                    self.sync_schedule_tick(effects);
                } else {
                    self.scheduled_tick = None;
                    self.emit_status(self.dirty_without_eligible_status(), effects);
                }
            }
            AutosaveState::Error {
                attempt,
                retry_at,
                error,
            } => {
                if !has_eligible {
                    *retry_at = None;
                    self.scheduled_tick = None;
                } else if retry_at.is_none() && is_retryable_error(&error.kind) {
                    let next_at = self
                        .policy
                        .retry_delay(*attempt)
                        .map(|delay| now.saturating_add(delay));
                    *retry_at = next_at;
                    self.scheduled_tick = next_at;
                    if let Some(at) = next_at {
                        effects.push(AutosaveEffect::ScheduleTick(at));
                    }
                }
            }
            AutosaveState::Idle => {
                if self.dirty_sections.is_empty() {
                    self.emit_status(AutosaveStatus::idle(), effects);
                }
            }
            AutosaveState::Saved { .. }
            | AutosaveState::Saving { .. }
            | AutosaveState::Conflict { .. } => {}
        }
    }

    pub(super) fn on_edit(
        &mut self,
        section: AutosaveSection,
        revision: RevisionHash,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        if self.scope.can_autosave() && !self.scope.can_autosave_section(section) {
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
            return;
        }

        if section.save_target() == SaveTarget::FullProfile {
            self.latest_draft_revision = Some(revision.clone());
        }

        if !self.in_flight_covers(section) && self.matches_saved_revision(section, &revision) {
            let was_dirty = self.dirty_sections.remove(&section).is_some();
            if !was_dirty
                && self.in_flight.is_none()
                && matches!(self.state, AutosaveState::Saved { .. })
            {
                return;
            }
            if self.in_flight.is_none()
                && self.dirty_sections.is_empty()
                && !matches!(self.state, AutosaveState::Conflict { .. })
            {
                self.retry_attempt = 0;
                self.scheduled_tick = None;
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
            } else if matches!(self.state, AutosaveState::Dirty { .. }) {
                self.set_dirty_state();
                if self.scope.can_autosave() && self.earliest_eligible_deadline().is_some() {
                    self.sync_schedule_tick(effects);
                } else {
                    self.scheduled_tick = None;
                    self.emit_status(self.dirty_without_eligible_status(), effects);
                }
            } else if matches!(self.state, AutosaveState::Error { .. })
                && self.eligible_dirty_sections().is_empty()
            {
                if let AutosaveState::Error { retry_at, .. } = &mut self.state {
                    *retry_at = None;
                }
                self.scheduled_tick = None;
            }
            return;
        }

        let from_error = matches!(self.state, AutosaveState::Error { .. });
        if from_error {
            self.retry_attempt = 0;
            for (&sec, entry) in &mut self.dirty_sections {
                entry.since = now;
                entry.deadline = Self::compute_deadline_for(
                    &self.policy,
                    now,
                    now,
                    self.policy.debounce_for(sec),
                );
            }
        }
        let since = if from_error {
            now
        } else {
            self.dirty_sections.get(&section).map_or(now, |e| e.since)
        };
        let deadline =
            Self::compute_deadline_for(&self.policy, now, since, self.policy.debounce_for(section));
        self.next_edit_seq = self.next_edit_seq.wrapping_add(1);
        self.dirty_sections.insert(
            section,
            DirtyEntry {
                since,
                deadline,
                revision: revision.clone(),
                edit_seq: self.next_edit_seq,
            },
        );

        match &mut self.state {
            AutosaveState::Conflict {
                local, sections, ..
            } => {
                if section.save_target() == SaveTarget::FullProfile
                    || !sections
                        .iter()
                        .any(|s| s.save_target() == SaveTarget::FullProfile)
                {
                    *local = revision;
                }
                sections.insert(section);
            }
            AutosaveState::Saving { .. } => {
                self.emit_status(AutosaveStatus::saving(), effects);
            }
            AutosaveState::Idle
            | AutosaveState::Saved { .. }
            | AutosaveState::Dirty { .. }
            | AutosaveState::Error { .. } => {
                self.set_dirty_state();
                if self.scope.can_autosave() {
                    self.emit_status(AutosaveStatus::saving(), effects);
                    self.sync_schedule_tick(effects);
                } else {
                    self.scheduled_tick = None;
                    let status = self.inactive_status();
                    self.emit_status(status, effects);
                }
            }
        }
    }

    pub(super) fn on_tick(&mut self, now: AutosaveInstant, effects: &mut Vec<AutosaveEffect>) {
        match &self.state {
            AutosaveState::Idle | AutosaveState::Saving { .. } | AutosaveState::Conflict { .. } => {
            }
            AutosaveState::Saved { at, .. } => {
                if let Some(hold) = self.policy.saved_status_hold {
                    if now >= at.saturating_add(hold) {
                        self.scheduled_tick = None;
                        self.state = AutosaveState::Idle;
                        self.emit_status(AutosaveStatus::idle(), effects);
                    }
                }
            }
            AutosaveState::Error {
                retry_at: Some(at), ..
            } => {
                if now >= *at && self.scope.can_autosave() {
                    self.dispatch_all_eligible(false, effects);
                }
            }
            AutosaveState::Error { retry_at: None, .. } => {}
            AutosaveState::Dirty { .. } => {
                if !self.scope.can_autosave() {
                    self.scheduled_tick = None;
                    return;
                }
                let due: BTreeSet<AutosaveSection> = self
                    .dirty_sections
                    .iter()
                    .filter(|(sec, entry)| {
                        self.scope.can_autosave_section(**sec) && entry.deadline <= now
                    })
                    .map(|(sec, _)| *sec)
                    .collect();
                if due.is_empty() {
                    if self.eligible_dirty_sections().is_empty() {
                        self.scheduled_tick = None;
                    }
                    return;
                }
                self.dispatch_due(&due, false, effects);
            }
        }
    }

    pub(super) fn on_flush(&mut self, effects: &mut Vec<AutosaveEffect>) {
        if !self.scope.can_autosave() {
            return;
        }
        match self.state {
            AutosaveState::Idle | AutosaveState::Saved { .. } | AutosaveState::Conflict { .. } => {}
            AutosaveState::Saving { .. } => {
                if !self.eligible_dirty_sections().is_empty() {
                    if let Some(in_flight) = &mut self.in_flight {
                        in_flight.flush_queued = true;
                    }
                }
            }
            AutosaveState::Dirty { .. } | AutosaveState::Error { .. } => {
                self.dispatch_all_eligible(true, effects);
            }
        }
    }

    pub(super) fn on_retry_now(&mut self, effects: &mut Vec<AutosaveEffect>) {
        if matches!(self.state, AutosaveState::Error { .. }) && self.scope.can_autosave() {
            self.dispatch_all_eligible(false, effects);
        }
    }

    pub(super) fn on_save_succeeded(
        &mut self,
        id: SaveId,
        committed_rev: RevisionHash,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let Some(in_flight) = self.in_flight.take_if(|f| f.id == id) else {
            return;
        };

        self.retry_attempt = 0;
        if in_flight.target == SaveTarget::FullProfile {
            self.base_revision = Some(committed_rev.clone());
        }
        if committed_rev != in_flight.revision {
            self.update_pending_self_write(&in_flight.revision, &committed_rev);
        }
        for (sec, entry) in &in_flight.entries {
            let saved_rev = if sec.save_target() == SaveTarget::FullProfile {
                committed_rev.clone()
            } else {
                entry.revision.clone()
            };
            self.section_saved_revs.insert(*sec, saved_rev);
        }
        self.prune_clean_dirty_sections(in_flight.target, &in_flight.entries, &committed_rev);

        let eligible = self.eligible_dirty_sections();
        if !eligible.is_empty() && self.scope.can_autosave() {
            if in_flight.flush_queued {
                self.dispatch_all_eligible(true, effects);
                return;
            }
            let due: BTreeSet<AutosaveSection> = eligible
                .iter()
                .copied()
                .filter(|sec| {
                    self.dirty_sections
                        .get(sec)
                        .is_some_and(|e| e.deadline <= now)
                })
                .collect();
            if !due.is_empty() {
                self.dispatch_due(&due, false, effects);
                return;
            }
            self.set_dirty_state();
            self.emit_status(AutosaveStatus::saving(), effects);
            self.sync_schedule_tick(effects);
            return;
        }

        if !self.dirty_sections.is_empty() {
            self.set_dirty_state();
            self.scheduled_tick = None;
            let status = self.dirty_without_eligible_status();
            self.emit_status(status, effects);
            return;
        }

        match self.policy.saved_status_hold {
            Some(hold) if !hold.is_zero() => {
                let fade_at = now.saturating_add(hold);
                self.scheduled_tick = Some(fade_at);
                self.state = AutosaveState::Saved {
                    at: now,
                    rev: committed_rev,
                };
                self.emit_status(AutosaveStatus::saved(), effects);
                effects.push(AutosaveEffect::ScheduleTick(fade_at));
            }
            _ => {
                self.scheduled_tick = None;
                self.state = AutosaveState::Idle;
                self.emit_status(AutosaveStatus::idle(), effects);
            }
        }
    }

    pub(super) fn on_save_failed(
        &mut self,
        id: SaveId,
        error: ServiceError,
        now: AutosaveInstant,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let Some(in_flight) = self.in_flight.take_if(|f| f.id == id) else {
            return;
        };

        self.remove_pending_self_write(&in_flight.revision);
        for (sec, entry) in in_flight.entries {
            match self.dirty_sections.get_mut(&sec) {
                Some(newer) => newer.since = newer.since.min(entry.since),
                None => {
                    self.dirty_sections.insert(sec, entry);
                }
            }
        }

        if error.kind == ServiceErrorKind::Cancelled {
            let eligible = self.eligible_dirty_sections();
            if eligible.is_empty() || !self.scope.can_autosave() {
                self.scheduled_tick = None;
                let status = if self.dirty_sections.is_empty() {
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
            self.set_dirty_state();
            self.emit_status(AutosaveStatus::saving(), effects);
            self.sync_schedule_tick(effects);
            return;
        }

        self.retry_attempt = self.retry_attempt.saturating_add(1);
        let retry_at = if is_retryable_error(&error.kind)
            && self.scope.can_autosave()
            && !self.eligible_dirty_sections().is_empty()
        {
            self.policy
                .retry_delay(self.retry_attempt)
                .map(|delay| now.saturating_add(delay))
        } else {
            None
        };
        self.scheduled_tick = retry_at;
        let detail = Some(error.message.clone());
        self.state = AutosaveState::Error {
            attempt: self.retry_attempt,
            retry_at,
            error,
        };
        self.emit_status(AutosaveStatus::error("Failed to save", detail), effects);
        if let Some(at) = retry_at {
            effects.push(AutosaveEffect::ScheduleTick(at));
        }
    }
}

fn is_retryable_error(kind: &ServiceErrorKind) -> bool {
    matches!(
        kind,
        ServiceErrorKind::Io | ServiceErrorKind::Unavailable | ServiceErrorKind::Internal
    )
}
