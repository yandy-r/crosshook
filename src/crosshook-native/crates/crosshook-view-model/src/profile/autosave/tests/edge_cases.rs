use std::collections::BTreeSet;

use crate::{ServiceError, ServiceErrorKind};

use super::super::*;
use super::{make_vm, ms, rev, saved_scope};

#[test]
fn from_error_edit_reanchors_other_dirty_sections() {
    let mut vm = make_vm(AutosavePolicy::default());
    // Gamescope edited at t=0 (deadline 350); ProfileDraft edited at t=100 (deadline 450).
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Gamescope,
            revision: rev("gs-1"),
        },
        ms(0),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("bad-draft"),
        },
        ms(100),
    );
    // Tick at 350 triggers Gamescope save; fail with Validation at 360 -> Gamescope stays dirty (deadline 350).
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected Gamescope save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveFailed {
            id: req.id,
            error: ServiceError::validation("gamescope", "invalid"),
        },
        ms(360),
    );
    // Later at t=600, edit a different section (MangoHud): Gamescope/ProfileDraft deadlines must re-anchor >= 950.
    let effects = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::MangoHud,
            revision: rev("mh-1"),
        },
        ms(600),
    );
    assert_eq!(
        effects,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saving()),
            AutosaveEffect::ScheduleTick(ms(950)),
        ]
    );
    assert_eq!(vm.scheduled_tick(), Some(ms(950)));
    assert!(vm.apply(AutosaveEvent::Tick, ms(949)).is_empty());
}

#[test]
fn scope_changed_launch_method_transitions_and_mid_rename_invariants() {
    // ProtonRun -> Native while only LaunchOptimizations is dirty clears timer & warns.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchOptimizations,
            revision: rev("opt-1"),
        },
        ms(0),
    );
    assert_eq!(vm.scheduled_tick(), Some(ms(350)));
    let to_native = vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::Native)),
        ms(100),
    );
    assert_eq!(
        to_native,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::warning(
            "Unavailable for current method",
            None
        ))]
    );
    assert_eq!(vm.scheduled_tick(), None);
    // Flush & Tick respect idle gate while no eligible sections exist.
    assert!(vm.apply(AutosaveEvent::Flush, ms(200)).is_empty());
    assert!(vm.apply(AutosaveEvent::Tick, ms(350)).is_empty());

    // Native -> ProtonRun after past deadline clamps to now and arms tick immediately.
    let to_proton = vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::ProtonRun)),
        ms(600),
    );
    assert_eq!(
        to_proton,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saving()),
            AutosaveEffect::ScheduleTick(ms(600)),
        ]
    );
    assert_eq!(vm.scheduled_tick(), Some(ms(600)));

    // Native -> ProtonRun also clears stale warning when Idle and clean.
    let mut idle_native = ProfileAutosave::new(
        AutosaveScope::saved("Test Game", LaunchMethod::Native),
        Some(rev("r0")),
    );
    let warn = idle_native.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchOptimizations,
            revision: rev("opt-1"),
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
    let cleared = idle_native.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::ProtonRun)),
        ms(20),
    );
    assert_eq!(
        cleared,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::idle())]
    );

    // Mid-rename transitions Saved -> Idle cleanly and preserves Error/Conflict/Saving statuses.
    let mut saved_vm = make_vm(AutosavePolicy::default());
    let _ = saved_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &saved_vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = saved_vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("r1"),
        },
        ms(360),
    );
    assert!(matches!(saved_vm.state(), AutosaveState::Saved { .. }));
    let rename_scope = AutosaveScope {
        selected_profile: "Test Game".into(),
        draft_name: "Test Game 2".into(),
        profile_exists: true,
        launch_method: LaunchMethod::ProtonRun,
    };
    let mid_rename = saved_vm.apply(AutosaveEvent::ScopeChanged(rename_scope.clone()), ms(400));
    assert_eq!(
        mid_rename,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::idle())]
    );
    assert_eq!(saved_vm.state(), &AutosaveState::Idle);
    assert_eq!(saved_vm.scheduled_tick(), None);

    // Preserves Error status across mid-rename pause.
    let _ = saved_vm.apply(AutosaveEvent::ScopeChanged(saved_scope()), ms(410));
    let _ = saved_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r2"),
        },
        ms(420),
    );
    let AutosaveEffect::Save(req2) = &saved_vm.apply(AutosaveEvent::Tick, ms(770))[0] else {
        panic!("expected save 2");
    };
    let _ = saved_vm.apply(
        AutosaveEvent::SaveFailed {
            id: req2.id,
            error: ServiceError::new(ServiceErrorKind::Io, "io err"),
        },
        ms(780),
    );
    assert!(saved_vm
        .apply(AutosaveEvent::ScopeChanged(rename_scope), ms(790))
        .is_empty());
    assert_eq!(saved_vm.status().tone, AutosaveTone::Error);
    assert!(matches!(
        saved_vm.state(),
        AutosaveState::Error { retry_at: None, .. }
    ));

    // Ending mid-rename re-arms backoff for a paused retryable Io error, while Validation stays manual.
    let resumed_io = saved_vm.apply(AutosaveEvent::ScopeChanged(saved_scope()), ms(800));
    assert_eq!(resumed_io, vec![AutosaveEffect::ScheduleTick(ms(1_800))]);
    assert!(matches!(
        saved_vm.state(),
        AutosaveState::Error {
            retry_at: Some(at),
            ..
        } if *at == ms(1_800)
    ));

    let mut val_vm = make_vm(AutosavePolicy::default());
    let _ = val_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("bad"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(vreq) = &val_vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = val_vm.apply(
        AutosaveEvent::SaveFailed {
            id: vreq.id,
            error: ServiceError::validation("game", "bad"),
        },
        ms(360),
    );
    let val_rename = AutosaveScope {
        selected_profile: "Test Game".into(),
        draft_name: "Test Game 2".into(),
        profile_exists: true,
        launch_method: LaunchMethod::ProtonRun,
    };
    let _ = val_vm.apply(AutosaveEvent::ScopeChanged(val_rename), ms(370));
    assert!(val_vm
        .apply(AutosaveEvent::ScopeChanged(saved_scope()), ms(380))
        .is_empty());
    assert_eq!(val_vm.scheduled_tick(), None);
}

#[test]
fn ineligible_edit_and_noop_saved_edit_invariants() {
    // 1. Ineligible edit does not pollute latest_draft_revision and clears Saved hold before Warning.
    let mut vm = ProfileAutosave::new(
        AutosaveScope::saved("Native", LaunchMethod::Native),
        Some(rev("full-0")),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("full-1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("full-1"),
        },
        ms(360),
    );
    let warn = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::CommandArguments,
            revision: rev("cmd-bogus"),
        },
        ms(400),
    );
    assert_eq!(
        warn,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::warning(
            "Unavailable for current method",
            None
        ))]
    );
    assert_eq!(vm.latest_draft_revision(), Some(&rev("full-1")));
    assert_eq!(vm.state(), &AutosaveState::Idle);
    assert_eq!(vm.scheduled_tick(), None);
    // 3s fade tick must NOT overwrite Warning later.
    assert!(vm.apply(AutosaveEvent::Tick, ms(3_360)).is_empty());
    assert_eq!(vm.status().tone, AutosaveTone::Warning);

    // 2. No-op Edit while Saved preserves Saved state and 3s fade timer.
    let mut saved_vm = make_vm(AutosavePolicy::default());
    let _ = saved_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Environment,
            revision: rev("env-1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &saved_vm.apply(AutosaveEvent::Tick, ms(400))[0] else {
        panic!("expected save");
    };
    let _ = saved_vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("env-1"),
        },
        ms(410),
    );
    let blur_noop = saved_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Environment,
            revision: rev("env-1"),
        },
        ms(500),
    );
    assert!(blur_noop.is_empty());
    assert!(matches!(saved_vm.state(), AutosaveState::Saved { .. }));
    assert_eq!(saved_vm.scheduled_tick(), Some(ms(3_410)));
}

#[test]
fn section_revision_isolation_and_unsaved_cancelled_warning() {
    // Dedicated section saves do not clobber base_revision or prune unrelated sections with equal fingerprints.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Gamescope,
            revision: rev("shared-tag"),
        },
        ms(0),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::MangoHud,
            revision: rev("shared-tag"),
        },
        ms(50),
    );
    let AutosaveEffect::Save(gs_req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected Gamescope save");
    };
    assert_eq!(
        gs_req.target,
        SaveTarget::Section(SectionSaveKind::Gamescope)
    );
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: gs_req.id,
            committed_rev: rev("shared-tag"),
        },
        ms(360),
    );
    // base_revision stayed "r0" and MangoHud remained dirty!
    assert_eq!(vm.base_revision(), Some(&rev("r0")));
    assert_eq!(
        vm.dirty_sections(),
        BTreeSet::from([AutosaveSection::MangoHud])
    );

    // Editing ProfileDraft then Gamescope dispatches FullProfile carrying ProfileDraft's revision.
    let mut mixed = make_vm(AutosavePolicy::default());
    let _ = mixed.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("full-2"),
        },
        ms(0),
    );
    let _ = mixed.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Gamescope,
            revision: rev("gs-2"),
        },
        ms(10),
    );
    let AutosaveEffect::Save(full_req) = &mixed.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected FullProfile save");
    };
    assert_eq!(full_req.target, SaveTarget::FullProfile);
    assert_eq!(full_req.revision, rev("full-2"));

    // Cancelled save when profile_exists flips false emits "Save profile first".
    let mut unsaved_vm = make_vm(AutosavePolicy::default());
    let _ = unsaved_vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &unsaved_vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    let _ = unsaved_vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope {
            selected_profile: "Test Game".into(),
            draft_name: "Test Game".into(),
            profile_exists: false,
            launch_method: LaunchMethod::ProtonRun,
        }),
        ms(355),
    );
    let cancelled = unsaved_vm.apply(
        AutosaveEvent::SaveFailed {
            id: req.id,
            error: ServiceError::new(ServiceErrorKind::Cancelled, "cancelled"),
        },
        ms(360),
    );
    assert_eq!(
        cancelled,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::warning(
            "Save profile first",
            None
        ))]
    );
}
