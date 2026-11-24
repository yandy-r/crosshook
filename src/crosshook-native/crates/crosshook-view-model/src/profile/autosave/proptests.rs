use std::collections::BTreeMap;

use proptest::prelude::*;

use crate::{ServiceError, ServiceErrorKind};

use super::*;

#[derive(Debug, Clone)]
enum EditMode {
    Fresh,
    Repeat,
    UndoToBaseline,
}

#[derive(Debug, Clone)]
enum Step {
    Edit(AutosaveSection, EditMode),
    Advance(u16),
    Flush,
    SwitchMethod(bool),
    ToggleMidRename(bool),
    ExternalFavorite,
    ExternalOtherProfile,
}

fn arb_section() -> impl Strategy<Value = AutosaveSection> {
    prop_oneof![
        Just(AutosaveSection::ProfileDraft),
        Just(AutosaveSection::LaunchHooks),
        Just(AutosaveSection::Trainer),
        Just(AutosaveSection::Environment),
        Just(AutosaveSection::LaunchOptimizations),
        Just(AutosaveSection::Gamescope),
        Just(AutosaveSection::TrainerGamescope),
        Just(AutosaveSection::MangoHud),
        Just(AutosaveSection::CommandArguments),
    ]
}

fn arb_edit_mode() -> impl Strategy<Value = EditMode> {
    prop_oneof![
        Just(EditMode::Fresh),
        Just(EditMode::Repeat),
        Just(EditMode::UndoToBaseline),
    ]
}

fn arb_step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => (arb_section(), arb_edit_mode()).prop_map(|(s, m)| Step::Edit(s, m)),
        2 => (1u16..800).prop_map(Step::Advance),
        1 => Just(Step::Flush),
        1 => any::<bool>().prop_map(Step::SwitchMethod),
        1 => any::<bool>().prop_map(Step::ToggleMidRename),
        1 => Just(Step::ExternalFavorite),
        1 => Just(Step::ExternalOtherProfile),
    ]
}

struct ModelHarness {
    vm: ProfileAutosave,
    now: u64,
    rev_counter: u64,
    baseline: BTreeMap<AutosaveSection, RevisionHash>,
    draft_store: BTreeMap<AutosaveSection, RevisionHash>,
    disk_store: BTreeMap<AutosaveSection, RevisionHash>,
    in_flight: Option<(SaveRequest, BTreeMap<AutosaveSection, RevisionHash>)>,
    native_method: bool,
    mid_rename: bool,
}

impl ModelHarness {
    fn new(name: &str) -> Self {
        let baseline = AutosaveSection::ALL
            .iter()
            .map(|&s| (s, RevisionHash::new("init")))
            .collect::<BTreeMap<_, _>>();
        Self {
            vm: ProfileAutosave::new(
                AutosaveScope::saved(name, LaunchMethod::ProtonRun),
                Some(RevisionHash::new("init")),
            ),
            now: 0,
            rev_counter: 0,
            draft_store: baseline.clone(),
            disk_store: baseline.clone(),
            baseline,
            in_flight: None,
            native_method: false,
            mid_rename: false,
        }
    }

    fn current_scope(&self) -> AutosaveScope {
        AutosaveScope {
            selected_profile: "Prop Game".into(),
            draft_name: if self.mid_rename {
                "Prop Game Renamed".into()
            } else {
                "Prop Game".into()
            },
            profile_exists: true,
            launch_method: if self.native_method {
                LaunchMethod::Native
            } else {
                LaunchMethod::ProtonRun
            },
        }
    }

    fn capture_save(&mut self, effects: Vec<AutosaveEffect>) {
        let mut saves = Vec::new();
        for eff in effects {
            if let AutosaveEffect::Save(req) = eff {
                saves.push(req);
            }
        }
        assert!(saves.len() <= 1, "multiple saves in one step");
        if let Some(req) = saves.pop() {
            assert!(self.in_flight.is_none(), "second save while in flight");
            let mut snapshot = BTreeMap::new();
            match req.target {
                SaveTarget::FullProfile => snapshot.extend(self.draft_store.clone()),
                SaveTarget::Section(_) => {
                    for sec in &req.sections {
                        if let Some(rev) = self.draft_store.get(sec) {
                            snapshot.insert(*sec, rev.clone());
                        }
                    }
                }
            }
            self.in_flight = Some((req, snapshot));
        }
    }

    fn settle_in_flight(&mut self, fail_kind: Option<ServiceErrorKind>) {
        let Some((req, snapshot)) = self.in_flight.take() else {
            return;
        };
        let echo = self.vm.apply(
            AutosaveEvent::ExternalChange(ExternalChange {
                profile_name: Some(req.profile_name.clone()),
                reason: ProfileChangeKind::Saved,
                revision: Some(req.revision.clone()),
            }),
            AutosaveInstant::from_millis(self.now),
        );
        assert!(
            !echo
                .iter()
                .any(|e| matches!(e, AutosaveEffect::ShowConflict(_))),
            "self-write caused conflict: {echo:?}"
        );
        self.now += 1;
        let effects = match fail_kind {
            Some(kind) => self.vm.apply(
                AutosaveEvent::SaveFailed {
                    id: req.id,
                    error: ServiceError::new(kind, "failed"),
                },
                AutosaveInstant::from_millis(self.now),
            ),
            None => {
                for (sec, rev) in &snapshot {
                    self.disk_store.insert(*sec, rev.clone());
                    if req.target == SaveTarget::FullProfile
                        || sec.save_target() != SaveTarget::FullProfile
                    {
                        self.baseline.insert(*sec, rev.clone());
                    }
                }
                self.vm.apply(
                    AutosaveEvent::SaveSucceeded {
                        id: req.id,
                        committed_rev: req.revision,
                    },
                    AutosaveInstant::from_millis(self.now),
                )
            }
        };
        self.capture_save(effects);
    }

    fn drain_to_idle(&mut self) {
        self.native_method = false;
        self.mid_rename = false;
        let eff = self.vm.apply(
            AutosaveEvent::ScopeChanged(self.current_scope()),
            AutosaveInstant::from_millis(self.now),
        );
        self.capture_save(eff);

        for _ in 0..300 {
            if self.in_flight.is_some() {
                self.settle_in_flight(None);
                continue;
            }
            if let Some(tick_at) = self.vm.scheduled_tick() {
                self.now = self.now.max(tick_at.as_millis());
                let eff = self
                    .vm
                    .apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(self.now));
                self.capture_save(eff);
                continue;
            }
            break;
        }
    }
}

proptest! {
    #[test]
    fn reference_model_convergence_single_flight_and_idle_gate(
        steps in prop::collection::vec(arb_step(), 1..40),
        fail_first in any::<bool>(),
    ) {
        let mut h = ModelHarness::new("Prop Game");
        let mut failed_once = false;

        for step in steps {
            let eff = match step {
                Step::Edit(sec, mode) => {
                    let rev = match mode {
                        EditMode::Fresh => {
                            h.rev_counter += 1;
                            RevisionHash::new(format!("rev-{}", h.rev_counter))
                        }
                        EditMode::Repeat => h.draft_store[&sec].clone(),
                        EditMode::UndoToBaseline => h.baseline[&sec].clone(),
                    };
                    if h.vm.scope().can_autosave_section(sec) || !h.vm.scope().can_autosave() {
                        h.draft_store.insert(sec, rev.clone());
                    }
                    h.vm.apply(
                        AutosaveEvent::Edit { section: sec, revision: rev },
                        AutosaveInstant::from_millis(h.now),
                    )
                }
                Step::Advance(delta) => {
                    h.now = h.now.saturating_add(u64::from(delta));
                    let mut out = Vec::new();
                    while let Some(due) = h.vm.scheduled_tick() {
                        prop_assert!(
                            due.as_millis() >= h.now.saturating_sub(u64::from(delta)),
                            "tick moved backward"
                        );
                        if due.as_millis() <= h.now {
                            out.extend(h.vm.apply(AutosaveEvent::Tick, due));
                        } else {
                            break;
                        }
                    }
                    out
                }
                Step::Flush => h.vm.apply(AutosaveEvent::Flush, AutosaveInstant::from_millis(h.now)),
                Step::SwitchMethod(is_native) => {
                    h.native_method = is_native;
                    h.vm.apply(
                        AutosaveEvent::ScopeChanged(h.current_scope()),
                        AutosaveInstant::from_millis(h.now),
                    )
                }
                Step::ToggleMidRename(renaming) => {
                    h.mid_rename = renaming;
                    h.vm.apply(
                        AutosaveEvent::ScopeChanged(h.current_scope()),
                        AutosaveInstant::from_millis(h.now),
                    )
                }
                Step::ExternalFavorite => h.vm.apply(
                    AutosaveEvent::ExternalChange(ExternalChange {
                        profile_name: Some("Prop Game".into()),
                        reason: ProfileChangeKind::FavoriteToggled,
                        revision: None,
                    }),
                    AutosaveInstant::from_millis(h.now),
                ),
                Step::ExternalOtherProfile => h.vm.apply(
                    AutosaveEvent::ExternalChange(ExternalChange {
                        profile_name: Some("Other Game".into()),
                        reason: ProfileChangeKind::Saved,
                        revision: Some(RevisionHash::new("other")),
                    }),
                    AutosaveInstant::from_millis(h.now),
                ),
            };
            h.capture_save(eff);

            if h.in_flight.is_some() {
                let kind = (fail_first && !failed_once).then_some(ServiceErrorKind::Io);
                if kind.is_some() {
                    failed_once = true;
                }
                h.settle_in_flight(kind);
            }
        }

        h.drain_to_idle();
        prop_assert_eq!(h.vm.state(), &AutosaveState::Idle);
        prop_assert!(!h.vm.is_dirty());
        prop_assert_eq!(h.vm.scheduled_tick(), None);
        prop_assert!(h.vm.apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(h.now + 10_000)).is_empty());
        prop_assert!(h.vm.apply(AutosaveEvent::Flush, AutosaveInstant::from_millis(h.now + 10_001)).is_empty());
        prop_assert_eq!(h.disk_store, h.draft_store);
    }

    #[test]
    fn conflict_resolution_and_validation_recovery_converge(
        sec in arb_section(),
        keep_mine in any::<bool>(),
    ) {
        let mut h = ModelHarness::new("Prop Game");
        // 1. Trigger a Validation failure (non-retryable -> no scheduled tick).
        let eff = h.vm.apply(
            AutosaveEvent::Edit {
                section: sec,
                revision: RevisionHash::new("bad"),
            },
            AutosaveInstant::from_millis(0),
        );
        h.draft_store.insert(sec, RevisionHash::new("bad"));
        h.capture_save(eff);
        let eff = h.vm.apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(400));
        h.now = 400;
        h.capture_save(eff);
        h.settle_in_flight(Some(ServiceErrorKind::Validation { field: "game" }));
        prop_assert_eq!(h.vm.scheduled_tick(), None);
        prop_assert!(
            matches!(h.vm.state(), AutosaveState::Error { retry_at: None, .. }),
            "expected non-retryable Error state, got {:?}",
            h.vm.state()
        );

        // 2. Fix edit clears Error and triggers external conflict -> KeepMine vs ReloadTheirs.
        let eff = h.vm.apply(
            AutosaveEvent::Edit {
                section: sec,
                revision: RevisionHash::new("good"),
            },
            AutosaveInstant::from_millis(500),
        );
        h.now = 500;
        h.draft_store.insert(sec, RevisionHash::new("good"));
        h.capture_save(eff);
        let conflict = h.vm.apply(
            AutosaveEvent::ExternalChange(ExternalChange {
                profile_name: Some("Prop Game".into()),
                reason: ProfileChangeKind::Saved,
                revision: Some(RevisionHash::new("remote-disk")),
            }),
            AutosaveInstant::from_millis(550),
        );
        prop_assert!(conflict.iter().any(|e| matches!(e, AutosaveEffect::ShowConflict(_))));
        prop_assert_eq!(h.vm.scheduled_tick(), None);

        let res = if keep_mine {
            ConflictResolution::KeepMine
        } else {
            ConflictResolution::ReloadTheirs
        };
        let eff = h.vm.apply(AutosaveEvent::ResolveConflict(res), AutosaveInstant::from_millis(600));
        h.now = 600;
        if keep_mine {
            h.capture_save(eff);
            h.drain_to_idle();
            prop_assert_eq!(h.disk_store, h.draft_store);
        } else {
            prop_assert!(
                eff.contains(&AutosaveEffect::Reload { profile_name: "Prop Game".into() }),
                "expected Reload effect, got {eff:?}"
            );
            prop_assert!(!h.vm.is_dirty());
        }
        prop_assert_eq!(h.vm.scheduled_tick(), None);
    }
}
