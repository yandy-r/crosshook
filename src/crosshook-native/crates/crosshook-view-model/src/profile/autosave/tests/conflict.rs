use std::collections::BTreeSet;

use super::super::*;
use super::{make_vm, ms, rev};
use crate::launch::LaunchMethod;
use crate::testing::Script;

fn ext(
    profile_name: Option<&str>,
    reason: ProfileChangeKind,
    revision: Option<&str>,
) -> ExternalChange {
    ExternalChange {
        profile_name: profile_name.map(str::to_owned),
        reason,
        revision: revision.map(RevisionHash::new),
        origin_save_id: None,
    }
}

fn ext_ev(name: Option<&str>, reason: ProfileChangeKind, revision: Option<&str>) -> AutosaveEvent {
    AutosaveEvent::ExternalChange(ext(name, reason, revision))
}

#[test]
fn external_change_reloads_when_idle_and_conflicts_when_dirty_or_saving() {
    // External change while Idle -> Reload, no Conflict.
    let vm = Script::new(make_vm(AutosavePolicy::default()))
        .step(
            AutosaveIntent::at(
                ms(10),
                ext_ev(Some("Test Game"), ProfileChangeKind::Saved, Some("disk-r1")),
            ),
            [AutosaveEffect::Reload {
                profile_name: "Test Game".into(),
            }],
        )
        .assert_state(|vm| assert_eq!(vm.state(), &AutosaveState::Idle))
        .into_inner();
    assert_eq!(vm.base_revision(), Some(&rev("disk-r1")));

    // External change while Dirty -> Conflict, no auto-merge, KeepMine overwrites, ReloadTheirs reloads.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("local-r1"),
        },
        ms(0),
    );
    let conflict = vm.apply(
        ext_ev(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("remote-r2"),
        ),
        ms(50),
    );
    assert_eq!(
        conflict,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::conflict()),
            AutosaveEffect::ShowConflict(ConflictDetails {
                profile_name: "Test Game".into(),
                local: rev("local-r1"),
                remote: Some(rev("remote-r2")),
                sections: BTreeSet::from([AutosaveSection::ProfileDraft]),
                reason: ProfileChangeKind::Saved,
            }),
        ]
    );
    assert_eq!(vm.scheduled_tick(), None);

    // KeepMine emits FullProfile save with local revision.
    let keep = vm.apply(
        AutosaveEvent::ResolveConflict(ConflictResolution::KeepMine),
        ms(60),
    );
    assert_eq!(keep.len(), 2);
    assert!(matches!(
        &keep[1],
        AutosaveEffect::Save(SaveRequest {
            target: SaveTarget::FullProfile,
            revision,
            ..
        }) if revision == &rev("local-r1")
    ));

    // External change while Saving -> Conflict and ignores stale SaveSucceeded.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("local-r1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(in_flight) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let conflict = vm.apply(
        ext_ev(
            Some("Test Game"),
            ProfileChangeKind::RolledBack,
            Some("remote-r3"),
        ),
        ms(360),
    );
    assert!(matches!(
        conflict.as_slice(),
        [
            AutosaveEffect::ShowStatus(_),
            AutosaveEffect::ShowConflict(_)
        ]
    ));
    // Stale SaveSucceeded cannot leave Conflict.
    assert!(vm
        .apply(
            AutosaveEvent::SaveSucceeded {
                id: in_flight.id,
                committed_rev: rev("local-r1"),
            },
            ms(370),
        )
        .is_empty());
    assert!(matches!(vm.state(), AutosaveState::Conflict { .. }));

    // ReloadTheirs clears local dirty state and requests Reload.
    let reload = vm.apply(
        AutosaveEvent::ResolveConflict(ConflictResolution::ReloadTheirs),
        ms(380),
    );
    assert_eq!(
        reload,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::idle()),
            AutosaveEffect::Reload {
                profile_name: "Test Game".into()
            },
        ]
    );
    assert!(!vm.is_dirty());
}

#[test]
fn self_write_events_never_yield_conflict_or_reload() {
    // Case 1: ProfilesChanged arrives BEFORE SaveSucceeded while Saving.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("self-1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    assert!(vm
        .apply(
            ext_ev(Some("Test Game"), ProfileChangeKind::Saved, Some("self-1")),
            ms(360),
        )
        .is_empty());
    assert!(matches!(vm.state(), AutosaveState::Saving { .. }));
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("self-1"),
        },
        ms(370),
    );

    // Case 2: ProfilesChanged arrives AFTER user started a newer edit (Dirty).
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("self-2"),
        },
        ms(400),
    );
    let AutosaveEffect::Save(req2) = &vm.apply(AutosaveEvent::Tick, ms(750))[0] else {
        panic!("expected save 2");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req2.id,
            committed_rev: rev("self-2"),
        },
        ms(760),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("self-3"),
        },
        ms(770),
    );
    assert!(vm
        .apply(
            ext_ev(Some("Test Game"), ProfileChangeKind::Saved, Some("self-2")),
            ms(780),
        )
        .is_empty());
    assert!(matches!(vm.state(), AutosaveState::Dirty { .. }));

    // Case 3: FavoriteToggled and other profiles are always ignored.
    assert!(vm
        .apply(
            ext_ev(Some("Test Game"), ProfileChangeKind::FavoriteToggled, None),
            ms(790),
        )
        .is_empty());
    assert!(vm
        .apply(
            ext_ev(
                Some("Other Game"),
                ProfileChangeKind::Saved,
                Some("other-1")
            ),
            ms(800),
        )
        .is_empty());
}

#[test]
fn idle_gate_emits_no_schedule_tick_after_settling_or_while_idle() {
    let mut vm = make_vm(AutosavePolicy::default());
    assert!(vm.apply(AutosaveEvent::Tick, ms(1_000)).is_empty());
    assert!(vm.apply(AutosaveEvent::Flush, ms(1_000)).is_empty());
    assert_eq!(vm.scheduled_tick(), None);

    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("r1"),
        },
        ms(360),
    );
    assert_eq!(vm.scheduled_tick(), Some(ms(3_360)));

    // Saved hold expires -> Idle with zero further ScheduleTick effects.
    let faded = vm.apply(AutosaveEvent::Tick, ms(3_360));
    assert_eq!(
        faded,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::idle())]
    );
    assert_eq!(vm.state(), &AutosaveState::Idle);
    assert_eq!(vm.scheduled_tick(), None);
    assert!(vm.apply(AutosaveEvent::Tick, ms(10_000)).is_empty());
}

#[test]
fn rejects_stale_save_ids_across_instances_and_profile_switches() {
    let mut vm_a = make_vm(AutosavePolicy::default());
    let mut vm_b = make_vm(AutosavePolicy::default());
    let _ = vm_a.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("a1"),
        },
        ms(0),
    );
    let _ = vm_b.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("b1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req_a) = &vm_a.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save a");
    };
    let AutosaveEffect::Save(req_b) = &vm_b.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save b");
    };

    // Instance B rejects instance A's SaveId.
    assert!(vm_b
        .apply(
            AutosaveEvent::SaveSucceeded {
                id: req_a.id,
                committed_rev: rev("a1"),
            },
            ms(360),
        )
        .is_empty());
    assert!(matches!(vm_b.state(), AutosaveState::Saving { .. }));
    assert!(req_b.id.mount() != req_a.id.mount());
    assert!(req_b.id.seq() == 1);

    // Switching profiles ignores late reply for previous profile.
    let _ = vm_b.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved(
            "Another Game",
            LaunchMethod::ProtonRun,
        )),
        ms(365),
    );
    assert!(vm_b
        .apply(
            AutosaveEvent::SaveSucceeded {
                id: req_b.id,
                committed_rev: rev("b1"),
            },
            ms(370),
        )
        .is_empty());
}

#[test]
fn undo_back_to_baseline_and_section_status_gating() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    assert!(vm.is_dirty());
    // Editing back to baseline "r0" clears dirty without saving.
    let undone = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r0"),
        },
        ms(100),
    );
    assert_eq!(
        undone,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::idle())]
    );
    assert!(!vm.is_dirty());
    assert_eq!(vm.scheduled_tick(), None);

    // Native launch method gates optimizations and command arguments.
    let native_vm = ProfileAutosave::new(
        AutosaveScope::saved("Native Game", LaunchMethod::Native),
        Some(rev("n0")),
    );
    assert_eq!(
        native_vm.section_status(AutosaveSection::LaunchOptimizations),
        AutosaveStatus::warning("Unavailable for current method", None)
    );
    assert_eq!(
        native_vm.section_status(AutosaveSection::Gamescope),
        AutosaveStatus::idle()
    );
}

#[test]
fn combine_statuses_matches_classic_priority_and_tie_break() {
    let err = AutosaveStatus::error("Err", None);
    let saving = AutosaveStatus::saving();
    let warn = AutosaveStatus::warning("Warn", None);
    let ok = AutosaveStatus::saved();
    let idle = AutosaveStatus::idle();
    let warn2 = AutosaveStatus::warning("Second warn", None);

    assert_eq!(combine_statuses([&idle, &ok, &warn, &saving, &err]), err);
    assert_eq!(combine_statuses([&idle, &ok, &warn, &saving]), saving);
    assert_eq!(combine_statuses([&idle, &warn, &warn2, &ok]), warn);
    assert_eq!(combine_statuses([&idle, &ok]), ok);
    assert_eq!(combine_statuses([&idle]), idle);
}

#[test]
fn external_renamed_from_active_reloads_clean_or_conflicts_when_dirty() {
    use crate::{ServiceError, ServiceErrorKind};

    // 1. Clean Idle -> Reloads the new profile name instead of ignoring Renamed { from }.
    let mut clean = make_vm(AutosavePolicy::default());
    let rename = ProfileChangeKind::Renamed {
        from: "Test Game".into(),
    };
    let reload = clean.apply(
        ext_ev(Some("Renamed Game"), rename.clone(), Some("r2")),
        ms(10),
    );
    assert_eq!(
        reload,
        vec![AutosaveEffect::Reload {
            profile_name: "Renamed Game".into()
        }]
    );

    // 2. Dirty -> Enters Conflict showing new name; ReloadTheirs reloads new name.
    let mut dirty = make_vm(AutosavePolicy::default());
    let _ = dirty.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("local-r1"),
        },
        ms(0),
    );
    let conflict = dirty.apply(
        ext_ev(Some("Renamed Game"), rename.clone(), Some("remote-r2")),
        ms(50),
    );
    assert_eq!(
        conflict,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::conflict()),
            AutosaveEffect::ShowConflict(ConflictDetails {
                profile_name: "Renamed Game".into(),
                local: rev("local-r1"),
                remote: Some(rev("remote-r2")),
                sections: BTreeSet::from([AutosaveSection::ProfileDraft]),
                reason: rename.clone(),
            }),
        ]
    );
    let theirs = dirty.apply(
        AutosaveEvent::ResolveConflict(ConflictResolution::ReloadTheirs),
        ms(60),
    );
    assert_eq!(
        theirs,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::idle()),
            AutosaveEffect::Reload {
                profile_name: "Renamed Game".into()
            },
        ]
    );

    // 3. KeepMine while active save is in flight defers until in-flight settles and targets the renamed profile.
    let mut in_flight_vm = make_vm(AutosavePolicy::default());
    let _ = in_flight_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("local-r1"),
        },
        ms(100),
    );
    let AutosaveEffect::Save(first_req) = &in_flight_vm.apply(AutosaveEvent::Tick, ms(450))[0]
    else {
        panic!("expected initial save");
    };
    let _ = in_flight_vm.apply(
        ext_ev(Some("Renamed Game"), rename, Some("remote-r2")),
        ms(460),
    );
    assert!(in_flight_vm
        .apply(
            AutosaveEvent::ResolveConflict(ConflictResolution::KeepMine),
            ms(500),
        )
        .is_empty());
    let resumed = in_flight_vm.apply(
        AutosaveEvent::SaveFailed {
            id: first_req.id,
            error: ServiceError::new(ServiceErrorKind::Io, "disk busy"),
        },
        ms(505),
    );
    let AutosaveEffect::Save(req) = &resumed[1] else {
        panic!("expected deferred KeepMine Save after first save settled, got {resumed:?}");
    };
    assert_eq!(req.profile_name, "Renamed Game");
    assert_eq!(in_flight_vm.scope().profile_name(), "Renamed Game");
    let _ = in_flight_vm.apply(
        AutosaveEvent::SaveFailed {
            id: req.id,
            error: ServiceError::new(ServiceErrorKind::Cancelled, "cancelled"),
        },
        ms(520),
    );
    assert!(matches!(
        in_flight_vm.state(),
        AutosaveState::Dirty { since, .. } if *since == ms(100)
    ));
}
