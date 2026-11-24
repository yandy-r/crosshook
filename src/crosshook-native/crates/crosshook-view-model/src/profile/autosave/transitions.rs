//! Core state transitions for [`super::ProfileAutosave`] (load, scope, edit,
//! tick, flush, save completion).

use std::collections::BTreeSet;

use crate::{ServiceError, ServiceErrorKind};

use super::effects::AutosaveEffect;
use super::events::ProfileChangeKind;
use super::helpers::is_retryable_error;
use super::policy::AutosaveStatus;
use super::state::{
    AutosaveInstant, AutosaveScope, AutosaveSection, AutosaveState, ReloadId, RevisionHash, SaveId,
    SaveTarget,
};
use super::{DirtyEntry, ProfileAutosave};

impl ProfileAutosave {
    pub(super) fn on_loaded(
        &mut self,
        scope: AutosaveScope,
        revision: RevisionHash,
        reload_id: Option<ReloadId>,
        effects: &mut Vec<AutosaveEffect>,
    ) {
        let loaded_name = scope.profile_name().to_owned();
        if let Some(id) = reload_id {
            let Some(reload) = self.pending_reload.take_if(|pending| pending.id == id) else {
                return;
            };
            if matches!(self.state, AutosaveState::Conflict { .. }) {
                return;
            }
            if self.is_dirty() {
                self.on_loaded_conflict(loaded_name, revision, reload.reason, effects);
                return;
            }
            if self.last_save_seq != reload.save_seq_at_request {
                return;
            }
        } else {
            let active_name = self.scope.profile_name().to_owned();
            let is_same_profile = !active_name.is_empty()
                && (loaded_name == active_name
                    || self
                        .pending_reload
                        .as_ref()
                        .is_some_and(|pending| pending.profile_name == loaded_name));
            if is_same_profile {
                if matches!(self.state, AutosaveState::Conflict { .. }) {
                    return;
                }
                if self.is_dirty() {
                    let reason = self
                        .pending_reload
                        .take()
                        .map_or(ProfileChangeKind::Saved, |pending| pending.reason);
                    self.on_loaded_conflict(loaded_name, revision, reason, effects);
                    return;
                }
            }
        }

        self.reset_working_state();
        self.scope = scope;
        self.base_revision = Some(revision.clone());
        self.latest_draft_revision = Some(revision);
        self.disk_base_matches = true;
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
            self.disk_base_matches = false;
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
                    if self.pending_flush {
                        self.pending_flush = false;
                        self.dispatch_all_eligible(false, effects);
                    } else {
                        self.emit_status(AutosaveStatus::saving(), effects);
                        self.sync_schedule_tick(effects);
                    }
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
        if !self.scope.method_supports_section(section) {
            self.handle_method_gated_edit(section, &revision, now, effects);
            return;
        }

        if section.save_target() == SaveTarget::FullProfile {
            self.latest_draft_revision = Some(revision.clone());
        }

        if !matches!(self.state, AutosaveState::Conflict { .. })
            && !self.in_flight_covers(section)
            && self.matches_saved_revision(section, &revision)
        {
            let was_dirty = self.dirty_sections.remove(&section).is_some();
            self.failed_sections.remove(&section);
            if !was_dirty
                && self.in_flight.is_none()
                && matches!(self.state, AutosaveState::Saved { .. })
            {
                return;
            }
            self.finish_undo_section(now, effects);
            return;
        }

        let from_error = matches!(self.state, AutosaveState::Error { .. });
        if from_error {
            self.retry_attempt = 0;
            self.failed_sections.clear();
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
            AutosaveState::Conflict { .. } => {
                self.record_conflict_edit(section, revision);
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
                    self.pending_flush = true;
                }
            }
            AutosaveState::Dirty { .. } | AutosaveState::Error { .. } => {
                self.pending_flush = false;
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
        if matches!(self.state, AutosaveState::Conflict { .. }) {
            self.settle_in_flight_during_conflict(in_flight, Some(&committed_rev), now, effects);
            return;
        }

        self.retry_attempt = 0;
        self.failed_sections.clear();
        if in_flight.target == SaveTarget::FullProfile {
            self.base_revision = Some(committed_rev.clone());
            self.disk_base_matches = true;
            self.section_saved_revs.clear();
        } else {
            self.disk_base_matches = false;
        }
        if committed_rev != in_flight.revision {
            self.update_pending_self_write(in_flight.id, &committed_rev);
        }
        for (sec, entry) in &in_flight.entries {
            if sec.save_target() != SaveTarget::FullProfile {
                self.section_saved_revs.insert(*sec, entry.revision.clone());
            }
        }
        self.prune_clean_dirty_sections(in_flight.target, &in_flight.entries, &committed_rev);
        if in_flight.target == SaveTarget::FullProfile
            && !self
                .dirty_sections
                .keys()
                .any(|s| s.save_target() == SaveTarget::FullProfile)
        {
            self.latest_draft_revision = Some(committed_rev.clone());
        }

        let eligible = self.eligible_dirty_sections();
        if !eligible.is_empty() && self.scope.can_autosave() {
            if self.pending_flush {
                self.pending_flush = false;
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

        self.pending_flush = false;
        if !self.scope.can_autosave() {
            self.scheduled_tick = None;
            self.state = AutosaveState::Idle;
            self.emit_status(self.inactive_status(), effects);
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
        if matches!(self.state, AutosaveState::Conflict { .. }) {
            self.settle_in_flight_during_conflict(in_flight, None, now, effects);
            return;
        }

        self.remove_pending_self_write_by_id(in_flight.id);
        let failed_secs: BTreeSet<AutosaveSection> = in_flight.entries.keys().copied().collect();
        for (sec, entry) in in_flight.entries {
            match self.dirty_sections.get_mut(&sec) {
                Some(newer) => newer.since = newer.since.min(entry.since),
                None => {
                    self.dirty_sections.insert(sec, entry);
                }
            }
        }

        if error.kind == ServiceErrorKind::Cancelled {
            self.handle_cancelled_save(now, effects);
            return;
        }

        self.pending_flush = false;

        self.failed_sections = failed_secs;
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
