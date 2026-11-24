use std::collections::BTreeSet;

use crate::launch::LaunchMethod;
use crate::{ServiceError, ServiceErrorKind};

use super::super::*;
use super::{make_vm, ms, rev};

fn ext(
    profile_name: Option<&str>,
    reason: ProfileChangeKind,
    revision: Option<&str>,
    origin_save_id: Option<SaveId>,
) -> AutosaveEvent {
    AutosaveEvent::ExternalChange(ExternalChange {
        profile_name: profile_name.map(str::to_owned),
        reason,
        revision: revision.map(RevisionHash::new),
        origin_save_id,
    })
}

fn edit(section: AutosaveSection, r: &str) -> AutosaveEvent {
    AutosaveEvent::Edit {
        section,
        revision: rev(r),
    }
}

fn loaded(name: &str, r: &str) -> AutosaveEvent {
    AutosaveEvent::Loaded {
        scope: AutosaveScope::saved(name, LaunchMethod::ProtonRun),
        revision: rev(r),
    }
}

#[test]
fn origin_save_id_suppresses_normalized_self_write_in_either_order_without_suppressing_external() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(edit(AutosaveSection::ProfileDraft, "draft-1"), ms(0));
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };

    // Normalized `Saved` arrives BEFORE `SaveSucceeded`, carrying matching `origin_save_id`.
    let before = vm.apply(
        ext(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("norm-1"),
            Some(req.id),
        ),
        ms(360),
    );
    assert!(before.is_empty());
    assert!(matches!(vm.state(), AutosaveState::Saving { .. }));

    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("norm-1"),
        },
        ms(370),
    );
    assert_eq!(vm.base_revision(), Some(&rev("norm-1")));

    // Foreign write with a different `origin_save_id` during a second save is NOT suppressed,
    // even when its revision matches the pending draft revision.
    let mut other_vm = make_vm(AutosavePolicy::default());
    let _ = other_vm.apply(edit(AutosaveSection::ProfileDraft, "foreign-draft"), ms(0));
    let AutosaveEffect::Save(foreign_req) = &other_vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected foreign save");
    };

    let _ = vm.apply(edit(AutosaveSection::ProfileDraft, "draft-2"), ms(400));
    let AutosaveEffect::Save(req2) = &vm.apply(AutosaveEvent::Tick, ms(750))[0] else {
        panic!("expected second save");
    };
    let conflict = vm.apply(
        ext(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("draft-2"),
            Some(foreign_req.id),
        ),
        ms(760),
    );
    assert!(matches!(
        conflict.as_slice(),
        [
            AutosaveEffect::ShowStatus(_),
            AutosaveEffect::ShowConflict(_)
        ]
    ));
    let _ = req2;

    // Untagged notifications after `SaveSucceeded` still match the updated committed revision.
    let mut vm3 = make_vm(AutosavePolicy::default());
    let _ = vm3.apply(edit(AutosaveSection::ProfileDraft, "draft-3"), ms(0));
    let AutosaveEffect::Save(req3) = &vm3.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected third save");
    };
    let _ = vm3.apply(
        AutosaveEvent::SaveSucceeded {
            id: req3.id,
            committed_rev: rev("norm-3"),
        },
        ms(360),
    );
    assert!(vm3
        .apply(
            ext(
                Some("Test Game"),
                ProfileChangeKind::Saved,
                Some("norm-3"),
                None
            ),
            ms(370),
        )
        .is_empty());
}

#[test]
fn keep_mine_after_external_rename_saves_to_conflict_profile_and_updates_scope() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(edit(AutosaveSection::ProfileDraft, "local-r1"), ms(0));
    let _ = vm.apply(
        ext(
            Some("Renamed Game"),
            ProfileChangeKind::Renamed {
                from: "Test Game".into(),
            },
            Some("remote-r2"),
            None,
        ),
        ms(50),
    );

    let keep = vm.apply(
        AutosaveEvent::ResolveConflict(ConflictResolution::KeepMine),
        ms(60),
    );
    let AutosaveEffect::Save(req) = &keep[1] else {
        panic!("expected KeepMine Save, got {keep:?}");
    };
    assert_eq!(req.profile_name, "Renamed Game");
    assert_eq!(req.target, SaveTarget::FullProfile);
    assert_eq!(vm.scope().profile_name(), "Renamed Game");
    assert_eq!(vm.scope().draft_name, "Renamed Game");

    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("local-r1"),
        },
        ms(70),
    );
    assert!(vm
        .apply(
            ext(
                Some("Renamed Game"),
                ProfileChangeKind::Saved,
                Some("local-r1"),
                Some(req.id),
            ),
            ms(80),
        )
        .is_empty());
}

#[test]
fn gated_edit_while_in_flight_is_retained_and_saved_on_method_restore() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(edit(AutosaveSection::LaunchOptimizations, "opt-1"), ms(0));
    let AutosaveEffect::Save(req1) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected initial save");
    };

    let _ = vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::Native)),
        ms(355),
    );
    assert!(vm
        .apply(edit(AutosaveSection::LaunchOptimizations, "opt-2"), ms(360))
        .is_empty());

    let after_first = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req1.id,
            committed_rev: rev("opt-1"),
        },
        ms(370),
    );
    assert_eq!(
        after_first,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::warning(
            "Unavailable for current method",
            None,
        ))]
    );
    assert!(vm.is_dirty());
    assert_eq!(vm.scheduled_tick(), None);

    let restored = vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::ProtonRun)),
        ms(500),
    );
    assert_eq!(
        restored,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saving()),
            AutosaveEffect::ScheduleTick(ms(710)),
        ]
    );
    let flushed = vm.apply(AutosaveEvent::Tick, ms(710));
    let AutosaveEffect::Save(req2) = &flushed[0] else {
        panic!("expected follow-up save, got {flushed:?}");
    };
    assert_eq!(req2.revision, rev("opt-2"));
}

#[test]
fn loaded_during_in_flight_save_preserves_single_flight_and_settles_in_conflict() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(edit(AutosaveSection::ProfileDraft, "local-1"), ms(0));
    let AutosaveEffect::Save(req1) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };

    let loaded_eff = vm.apply(loaded("Test Game", "disk-external"), ms(360));
    assert_eq!(
        loaded_eff,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::conflict()),
            AutosaveEffect::ShowConflict(ConflictDetails {
                profile_name: "Test Game".into(),
                local: rev("local-1"),
                remote: Some(rev("disk-external")),
                sections: BTreeSet::from([AutosaveSection::ProfileDraft]),
                reason: ProfileChangeKind::Saved,
            }),
        ]
    );

    // Further edits or Flush while req1 is in flight never spawn a second concurrent Save.
    assert!(vm
        .apply(edit(AutosaveSection::ProfileDraft, "local-2"), ms(365))
        .is_empty());
    assert!(vm.apply(AutosaveEvent::Flush, ms(366)).is_empty());
    assert_eq!(vm.scheduled_tick(), None);

    // ReloadTheirs stays deferred until req1 completes.
    assert!(vm
        .apply(
            AutosaveEvent::ResolveConflict(ConflictResolution::ReloadTheirs),
            ms(370),
        )
        .is_empty());
    let settled = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req1.id,
            committed_rev: rev("local-1"),
        },
        ms(380),
    );
    assert_eq!(
        settled,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::idle()),
            AutosaveEffect::Reload {
                profile_name: "Test Game".into(),
            },
        ]
    );
}

#[test]
fn loaded_overtaken_by_intervening_edit_conflicts_and_stale_reload_after_save_is_ignored() {
    // 1. Intervening edit while Reload is running -> Loaded surfaces Conflict instead of discarding edit.
    let mut vm = make_vm(AutosavePolicy::default());
    let reload = vm.apply(
        ext(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("remote-r1"),
            None,
        ),
        ms(10),
    );
    assert_eq!(
        reload,
        vec![AutosaveEffect::Reload {
            profile_name: "Test Game".into(),
        }]
    );

    let _ = vm.apply(edit(AutosaveSection::ProfileDraft, "local-r2"), ms(20));
    let conflict = vm.apply(loaded("Test Game", "remote-r1"), ms(30));
    assert_eq!(
        conflict,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::conflict()),
            AutosaveEffect::ShowConflict(ConflictDetails {
                profile_name: "Test Game".into(),
                local: rev("local-r2"),
                remote: Some(rev("remote-r1")),
                sections: BTreeSet::from([AutosaveSection::ProfileDraft]),
                reason: ProfileChangeKind::Saved,
            }),
        ]
    );

    // 2. If the intervening edit finished saving before Loaded returned, stale Loaded is ignored.
    let mut vm2 = make_vm(AutosavePolicy::default());
    let _ = vm2.apply(
        ext(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("remote-r1"),
            None,
        ),
        ms(10),
    );
    let _ = vm2.apply(edit(AutosaveSection::ProfileDraft, "local-r2"), ms(20));
    let AutosaveEffect::Save(req) = &vm2.apply(AutosaveEvent::Tick, ms(370))[0] else {
        panic!("expected save");
    };
    let _ = vm2.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("local-r2"),
        },
        ms(380),
    );
    assert!(vm2
        .apply(loaded("Test Game", "remote-r1"), ms(390))
        .is_empty());
    assert_eq!(vm2.base_revision(), Some(&rev("local-r2")));
}

#[test]
fn undo_clearing_last_failed_section_schedules_remaining_dirty_while_partial_undo_keeps_error() {
    // Part 1: Undoing the sole failed section (with a saved section rev) schedules remaining dirty MangoHud.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(edit(AutosaveSection::Gamescope, "gs-0"), ms(0));
    let AutosaveEffect::Save(seed_req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected seed Gamescope save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: seed_req.id,
            committed_rev: rev("gs-0"),
        },
        ms(360),
    );
    let _ = vm.apply(edit(AutosaveSection::Gamescope, "gs-1"), ms(1_000));
    let AutosaveEffect::Save(gs_req) = &vm.apply(AutosaveEvent::Tick, ms(1_350))[0] else {
        panic!("expected Gamescope save");
    };
    let _ = vm.apply(edit(AutosaveSection::MangoHud, "mh-1"), ms(1_360));
    let _ = vm.apply(
        AutosaveEvent::SaveFailed {
            id: gs_req.id,
            error: ServiceError::validation("gamescope", "invalid"),
        },
        ms(1_370),
    );
    assert!(matches!(vm.state(), AutosaveState::Error { .. }));

    let undone = vm.apply(edit(AutosaveSection::Gamescope, "gs-0"), ms(1_400));
    assert_eq!(
        undone,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saving()),
            AutosaveEffect::ScheduleTick(ms(1_710)),
        ]
    );
    assert!(matches!(vm.state(), AutosaveState::Dirty { .. }));
    assert_eq!(
        vm.dirty_sections(),
        BTreeSet::from([AutosaveSection::MangoHud])
    );
    let flushed = vm.apply(AutosaveEvent::Tick, ms(1_710));
    let AutosaveEffect::Save(mh_req) = &flushed[0] else {
        panic!("expected MangoHud save");
    };
    assert_eq!(
        mh_req.target,
        SaveTarget::Section(SectionSaveKind::MangoHud)
    );

    // Part 2: Partial undo while another failed section stays dirty keeps Error + retry policy.
    let mut vm2 = make_vm(AutosavePolicy::default());
    let _ = vm2.apply(edit(AutosaveSection::ProfileDraft, "r1"), ms(0));
    let _ = vm2.apply(edit(AutosaveSection::LaunchHooks, "r1"), ms(10));
    let AutosaveEffect::Save(init_req) = &vm2.apply(AutosaveEvent::Tick, ms(360))[0] else {
        panic!("expected initial save");
    };
    let _ = vm2.apply(
        AutosaveEvent::SaveSucceeded {
            id: init_req.id,
            committed_rev: rev("r1"),
        },
        ms(370),
    );
    let _ = vm2.apply(edit(AutosaveSection::ProfileDraft, "r2"), ms(400));
    let _ = vm2.apply(edit(AutosaveSection::LaunchHooks, "r2"), ms(410));
    let AutosaveEffect::Save(full_req) = &vm2.apply(AutosaveEvent::Tick, ms(750))[0] else {
        panic!("expected FullProfile save");
    };
    let _ = vm2.apply(
        AutosaveEvent::SaveFailed {
            id: full_req.id,
            error: ServiceError::new(ServiceErrorKind::Io, "disk full"),
        },
        ms(760),
    );
    assert!(vm2
        .apply(edit(AutosaveSection::LaunchHooks, "r1"), ms(800))
        .is_empty());
    assert!(matches!(
        vm2.state(),
        AutosaveState::Error {
            attempt: 1,
            retry_at: Some(_),
            ..
        }
    ));
}

#[test]
fn cancelled_save_preserves_queued_flush_once_without_retry_loop_and_survives_pause_resume() {
    // 1. Queued flush survives one Cancelled error and dispatches once (no loop on second cancel).
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(edit(AutosaveSection::Gamescope, "gs-1"), ms(0));
    let AutosaveEffect::Save(req1) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected initial save");
    };
    let _ = vm.apply(edit(AutosaveSection::MangoHud, "mh-1"), ms(360));
    assert!(vm.apply(AutosaveEvent::Flush, ms(370)).is_empty());

    let after_cancel1 = vm.apply(
        AutosaveEvent::SaveFailed {
            id: req1.id,
            error: ServiceError::new(ServiceErrorKind::Cancelled, "cancelled"),
        },
        ms(380),
    );
    let AutosaveEffect::Save(req2) = &after_cancel1[0] else {
        panic!("expected one queued flush Save, got {after_cancel1:?}");
    };
    let after_cancel2 = vm.apply(
        AutosaveEvent::SaveFailed {
            id: req2.id,
            error: ServiceError::new(ServiceErrorKind::Cancelled, "cancelled again"),
        },
        ms(390),
    );
    assert_eq!(after_cancel2, vec![AutosaveEffect::ScheduleTick(ms(740))]);
    assert!(matches!(vm.state(), AutosaveState::Dirty { .. }));

    // 2. Queued flush across pause -> cancel -> resume dispatches once on resume.
    let mut vm2 = make_vm(AutosavePolicy::default());
    let _ = vm2.apply(edit(AutosaveSection::LaunchOptimizations, "opt-1"), ms(0));
    let AutosaveEffect::Save(opt_req) = &vm2.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected opt save");
    };
    let _ = vm2.apply(edit(AutosaveSection::CommandArguments, "cmd-1"), ms(360));
    let _ = vm2.apply(AutosaveEvent::Flush, ms(370));
    let _ = vm2.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::Native)),
        ms(375),
    );
    let _ = vm2.apply(
        AutosaveEvent::SaveFailed {
            id: opt_req.id,
            error: ServiceError::new(ServiceErrorKind::Cancelled, "cancelled during pause"),
        },
        ms(380),
    );
    let resumed = vm2.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::ProtonRun)),
        ms(400),
    );
    assert!(
        matches!(
            resumed.as_slice(),
            [AutosaveEffect::ShowStatus(_), AutosaveEffect::Save(_)]
        ),
        "expected preserved flush on resume, got {resumed:?}"
    );
}
