use std::collections::BTreeSet;

use crate::launch::LaunchMethod;
use crate::ServiceError;

use super::super::*;
use super::{make_vm, ms, rev};

#[test]
fn deferred_reload_theirs_waits_for_in_flight_and_conflict_tracks_renamed_name() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("local-r1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(in_flight) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected in-flight save");
    };
    let _ = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("Renamed Game".into()),
            reason: ProfileChangeKind::Renamed {
                from: "Test Game".into(),
            },
            revision: Some(rev("remote-r2")),
            origin_save_id: None,
        }),
        ms(360),
    );

    // Follow-up ExternalChange targeting the renamed name updates remote without getting ignored,
    // and Resync uses the conflict_reload_name rather than falling back to the old name.
    let followup = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("Renamed Game".into()),
            reason: ProfileChangeKind::Saved,
            revision: Some(rev("remote-r3")),
            origin_save_id: None,
        }),
        ms(365),
    );
    assert!(matches!(
        followup.as_slice(),
        [AutosaveEffect::ShowConflict(ConflictDetails {
            profile_name,
            remote: Some(remote),
            ..
        })] if profile_name == "Renamed Game" && remote == &rev("remote-r3")
    ));
    let resync = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: None,
            reason: ProfileChangeKind::Resync,
            revision: None,
            origin_save_id: None,
        }),
        ms(368),
    );
    assert!(matches!(
        resync.as_slice(),
        [AutosaveEffect::ShowConflict(ConflictDetails { profile_name, .. })]
            if profile_name == "Renamed Game"
    ));

    // ReloadTheirs while in-flight defers Reload until the active write finishes.
    assert!(vm
        .apply(
            AutosaveEvent::ResolveConflict(ConflictResolution::ReloadTheirs),
            ms(370),
        )
        .is_empty());
    let settled = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: in_flight.id,
            committed_rev: rev("local-r1"),
        },
        ms(380),
    );
    assert_eq!(
        settled,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::idle()),
            AutosaveEffect::Reload {
                profile_name: "Renamed Game".into()
            },
        ]
    );
    assert!(!vm.is_dirty());
}

#[test]
fn latest_draft_revision_syncs_with_committed_rev_and_keep_mine() {
    // 1. committed_rev != in_flight.revision syncs latest_draft_revision once clean.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("raw-1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("norm-1"),
        },
        ms(360),
    );
    assert_eq!(vm.base_revision(), Some(&rev("norm-1")));
    assert_eq!(vm.latest_draft_revision(), Some(&rev("norm-1")));

    // 2. Section-only conflict KeepMine updates both base_revision & latest_draft_revision.
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Gamescope,
            revision: rev("gs-1"),
        },
        ms(400),
    );
    let _ = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("Test Game".into()),
            reason: ProfileChangeKind::Saved,
            revision: Some(rev("remote-2")),
            origin_save_id: None,
        }),
        ms(450),
    );
    let keep = vm.apply(
        AutosaveEvent::ResolveConflict(ConflictResolution::KeepMine),
        ms(460),
    );
    let AutosaveEffect::Save(keep_req) = &keep[1] else {
        panic!("expected KeepMine save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: keep_req.id,
            committed_rev: rev("merged-disk-3"),
        },
        ms(480),
    );
    assert_eq!(vm.base_revision(), Some(&rev("merged-disk-3")));
    assert_eq!(vm.latest_draft_revision(), Some(&rev("merged-disk-3")));

    // Subsequent section edit + external change uses the new section revision, not stale "norm-1".
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::MangoHud,
            revision: rev("mh-1"),
        },
        ms(500),
    );
    let c2 = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("Test Game".into()),
            reason: ProfileChangeKind::Saved,
            revision: Some(rev("remote-4")),
            origin_save_id: None,
        }),
        ms(510),
    );
    assert!(matches!(
        &c2[1],
        AutosaveEffect::ShowConflict(ConflictDetails { local, .. }) if local == &rev("mh-1")
    ));
}

#[test]
fn gated_already_dirty_edits_and_unsaved_native_gating() {
    // 1. Edit LaunchOptimizations under ProtonRun -> switch Native -> update -> revert clears dirty.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchOptimizations,
            revision: rev("opt-1"),
        },
        ms(0),
    );
    let _ = vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::Native)),
        ms(100),
    );
    // Update while Native updates stored revision cleanly without arming a timer.
    assert!(vm
        .apply(
            AutosaveEvent::Edit {
                section: AutosaveSection::LaunchOptimizations,
                revision: rev("opt-2"),
            },
            ms(150),
        )
        .is_empty());
    assert_eq!(vm.scheduled_tick(), None);

    // Reverting to baseline "r0" prunes the entry and transitions to Idle.
    let reverted = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchOptimizations,
            revision: rev("r0"),
        },
        ms(200),
    );
    assert_eq!(
        reverted,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::idle())]
    );
    assert!(!vm.is_dirty());

    // 2. Method-gated section edit while !profile_exists && Native does not queue dirty state.
    let mut unsaved_native =
        ProfileAutosave::new(AutosaveScope::unsaved("Draft", LaunchMethod::Native), None);
    let warn = unsaved_native.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchOptimizations,
            revision: rev("opt-x"),
        },
        ms(10),
    );
    assert_eq!(
        warn,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::warning(
            "Unavailable for current method",
            None
        ))]
    );
    assert!(!unsaved_native.is_dirty());
}

#[test]
fn in_flight_prevents_false_undo_and_conflict_skips_baseline_prune() {
    // 1. While ProfileDraft ("r1") is in flight, editing LaunchHooks to baseline ("r0") must NOT be dropped.
    let mut vm = make_vm(AutosavePolicy::default());
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
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchHooks,
            revision: rev("r0"),
        },
        ms(360),
    );
    assert_eq!(
        vm.dirty_sections(),
        BTreeSet::from([AutosaveSection::ProfileDraft, AutosaveSection::LaunchHooks])
    );

    // 2. While in Conflict, editing back to pre-conflict "r0" does not drop from dirty_sections.
    let _ = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("Test Game".into()),
            reason: ProfileChangeKind::Saved,
            revision: Some(rev("remote-r2")),
            origin_save_id: None,
        }),
        ms(365),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchHooks,
            revision: rev("r0"),
        },
        ms(370),
    );
    assert!(vm.has_queued_dirty_section(AutosaveSection::LaunchHooks));
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("r1"),
        },
        ms(380),
    );
    assert!(matches!(vm.state(), AutosaveState::Conflict { .. }));
}

#[test]
fn section_status_isolates_failed_section_and_skips_saved_hold_when_paused() {
    // 1. Gamescope failure only marks Gamescope as Error; unattempted dirty MangoHud stays Saving.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Gamescope,
            revision: rev("gs-1"),
        },
        ms(0),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::MangoHud,
            revision: rev("mh-1"),
        },
        ms(100),
    );
    let AutosaveEffect::Save(gs_req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected Gamescope save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveFailed {
            id: gs_req.id,
            error: ServiceError::validation("gamescope", "invalid"),
        },
        ms(360),
    );
    assert_eq!(
        vm.section_status(AutosaveSection::Gamescope).tone,
        AutosaveTone::Error
    );
    assert_eq!(
        vm.section_status(AutosaveSection::MangoHud).tone,
        AutosaveTone::Saving
    );

    // 2. SaveSucceeded while mid-rename skips 3s Saved hold and transitions directly to Idle.
    let mut paused_vm = make_vm(AutosavePolicy::default());
    let _ = paused_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &paused_vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = paused_vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope {
            selected_profile: "Test Game".into(),
            draft_name: "Test Game (renaming)".into(),
            profile_exists: true,
            launch_method: LaunchMethod::ProtonRun,
        }),
        ms(355),
    );
    let done = paused_vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("r1"),
        },
        ms(360),
    );
    assert_eq!(
        done,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::idle())]
    );
    assert_eq!(paused_vm.state(), &AutosaveState::Idle);
    assert_eq!(paused_vm.scheduled_tick(), None);
}

#[test]
fn section_save_invalidates_baseline_echo_suppression_while_preserving_undo() {
    let mut vm = make_vm(AutosavePolicy::default());
    // Save a dedicated Gamescope section change; base_revision stays "r0" for FullProfile undo,
    // while disk_base_matches becomes false so external restore to "r0" is NOT suppressed.
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Gamescope,
            revision: rev("gs-1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(gs_req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected Gamescope save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: gs_req.id,
            committed_rev: rev("gs-1"),
        },
        ms(360),
    );

    let external_restore = vm.apply(
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("Test Game".into()),
            reason: ProfileChangeKind::Saved,
            revision: Some(rev("r0")),
            origin_save_id: None,
        }),
        ms(400),
    );
    assert_eq!(
        external_restore,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::idle()),
            AutosaveEffect::Reload {
                profile_name: "Test Game".into()
            },
        ]
    );
}
