use super::{make_vm, ms, rev, *};

#[test]
fn full_profile_save_covers_and_invalidates_omitted_section_baselines() {
    let mut vm = make_vm(AutosavePolicy::default());
    let edit_lo = |r: &str| AutosaveEvent::Edit {
        section: AutosaveSection::LaunchOptimizations,
        revision: rev(r),
    };
    let edit_pd = |r: &str| AutosaveEvent::Edit {
        section: AutosaveSection::ProfileDraft,
        revision: rev(r),
    };

    // Seed section_saved_revs[LaunchOptimizations] = "lo-1".
    let _ = vm.apply(edit_lo("lo-1"), ms(0));
    let AutosaveEffect::Save(seed_req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected LaunchOptimizations seed save");
    };
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: seed_req.id,
            committed_rev: rev("lo-1"),
        },
        ms(360),
    );

    // Dirty LaunchOptimizations ("lo-2") & ProfileDraft ("pd-1"), then switch to Native so
    // LaunchOptimizations becomes method-gated and omitted from FullProfile SaveRequest::entries.
    let _ = vm.apply(edit_lo("lo-2"), ms(400));
    let _ = vm.apply(edit_pd("pd-1"), ms(410));
    let _ = vm.apply(
        AutosaveEvent::ScopeChanged(AutosaveScope::saved("Test Game", LaunchMethod::Native)),
        ms(420),
    );
    let flushed = vm.apply(AutosaveEvent::Flush, ms(430));
    let Some(full_req) = flushed.iter().find_map(|eff| match eff {
        AutosaveEffect::Save(req) => Some(req),
        _ => None,
    }) else {
        panic!("expected FullProfile save");
    };
    assert_eq!(full_req.target, SaveTarget::FullProfile);

    // Editing gated LaunchOptimizations back to "lo-1" while FullProfile is in flight must stay dirty,
    // and completing FullProfile invalidates stale section_saved_revs so reverting later still saves.
    let _ = vm.apply(edit_lo("lo-1"), ms(440));
    assert!(vm.has_queued_dirty_section(AutosaveSection::LaunchOptimizations));
    let _ = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: full_req.id,
            committed_rev: rev("pd-1"),
        },
        ms(450),
    );
    assert!(vm.has_queued_dirty_section(AutosaveSection::LaunchOptimizations));

    let _ = vm.apply(edit_lo("lo-3"), ms(460));
    let _ = vm.apply(edit_lo("lo-1"), ms(470));
    assert!(vm.has_queued_dirty_section(AutosaveSection::LaunchOptimizations));
}

#[test]
fn pending_reload_from_external_rename_enters_conflict_on_tick_and_flush_instead_of_saving_old_name(
) {
    let ext_rename = || {
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: Some("B".into()),
            reason: ProfileChangeKind::Renamed { from: "A".into() },
            revision: Some(rev("remote-b")),
            origin_save_id: None,
        })
    };
    let edit_a = |r: &str| AutosaveEvent::Edit {
        section: AutosaveSection::ProfileDraft,
        revision: rev(r),
    };

    for use_flush in [false, true] {
        let mut vm = ProfileAutosave::new(
            AutosaveScope::saved("A", LaunchMethod::ProtonRun),
            Some(rev("r0")),
        );
        let reload_eff = vm.apply(ext_rename(), ms(10));
        let [AutosaveEffect::Reload {
            id: reload_id,
            profile_name,
        }] = reload_eff.as_slice()
        else {
            panic!("expected Reload B, got {reload_eff:?}");
        };
        assert_eq!(profile_name, "B");

        let _ = vm.apply(edit_a("local-edit"), ms(20));
        let eff = if use_flush {
            vm.apply(AutosaveEvent::Flush, ms(30))
        } else {
            vm.apply(AutosaveEvent::Tick, ms(370))
        };
        assert!(!eff.iter().any(|e| matches!(e, AutosaveEffect::Save(_))));
        assert_eq!(
            eff,
            vec![
                AutosaveEffect::ShowStatus(AutosaveStatus::conflict()),
                AutosaveEffect::ShowConflict(ConflictDetails {
                    profile_name: "B".into(),
                    local: rev("local-edit"),
                    remote: Some(rev("remote-b")),
                    sections: BTreeSet::from([AutosaveSection::ProfileDraft]),
                    reason: ProfileChangeKind::Renamed { from: "A".into() },
                }),
            ]
        );

        // Delayed Loaded("B") for the invalidated reload_id is ignored cleanly.
        assert!(vm
            .apply(
                AutosaveEvent::Loaded {
                    scope: AutosaveScope::saved("B", LaunchMethod::ProtonRun),
                    revision: rev("remote-b"),
                    reload_id: Some(*reload_id),
                },
                ms(380),
            )
            .is_empty());

        // KeepMine adopts B and saves FullProfile to B rather than stale name A.
        let kept = vm.apply(
            AutosaveEvent::ResolveConflict(ConflictResolution::KeepMine),
            ms(390),
        );
        let Some(req) = kept.iter().find_map(|e| match e {
            AutosaveEffect::Save(req) => Some(req),
            _ => None,
        }) else {
            panic!("expected Save on KeepMine, got {kept:?}");
        };
        assert_eq!(req.profile_name, "B");
        assert_eq!(req.target, SaveTarget::FullProfile);
        assert_eq!(vm.scope().profile_name(), "B");
    }
}

#[test]
fn loaded_overtaken_by_intervening_edit_conflicts_and_stale_reload_after_save_is_ignored() {
    let ext = |name: Option<&str>, reason: ProfileChangeKind, r: Option<&str>| {
        AutosaveEvent::ExternalChange(ExternalChange {
            profile_name: name.map(str::to_owned),
            reason,
            revision: r.map(rev),
            origin_save_id: None,
        })
    };
    let edit = |section: AutosaveSection, r: &str| AutosaveEvent::Edit {
        section,
        revision: rev(r),
    };
    let loaded = |name: &str, r: &str, reload_id: Option<ReloadId>| AutosaveEvent::Loaded {
        scope: AutosaveScope::saved(name, LaunchMethod::ProtonRun),
        revision: rev(r),
        reload_id,
    };

    // 1. Intervening edit while Reload is running -> Loaded surfaces Conflict instead of discarding edit.
    let mut vm = make_vm(AutosavePolicy::default());
    let reload = vm.apply(
        ext(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("remote-r1"),
        ),
        ms(10),
    );
    let [AutosaveEffect::Reload {
        id: reload_1,
        profile_name,
    }] = reload.as_slice()
    else {
        panic!("expected Reload, got {reload:?}");
    };
    assert_eq!(profile_name, "Test Game");

    let _ = vm.apply(edit(AutosaveSection::ProfileDraft, "local-r2"), ms(20));
    let conflict = vm.apply(loaded("Test Game", "remote-r1", Some(*reload_1)), ms(30));
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

    // 2. If debounce fires while Reload is still pending, it surfaces Conflict instead of saving,
    //    rejects the overtaken Loaded completion, and ignores it after KeepMine commits too.
    let mut vm2 = make_vm(AutosavePolicy::default());
    let reload2 = vm2.apply(
        ext(
            Some("Test Game"),
            ProfileChangeKind::Saved,
            Some("remote-r1"),
        ),
        ms(10),
    );
    let AutosaveEffect::Reload { id: reload_2, .. } = reload2[0] else {
        panic!("expected Reload 2");
    };
    let _ = vm2.apply(edit(AutosaveSection::ProfileDraft, "local-r2"), ms(20));
    let conflict2 = vm2.apply(AutosaveEvent::Tick, ms(370));
    assert!(!conflict2
        .iter()
        .any(|e| matches!(e, AutosaveEffect::Save(_))));
    assert_eq!(
        conflict2,
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
    assert!(vm2
        .apply(loaded("Test Game", "remote-r1", Some(reload_2)), ms(375))
        .is_empty());

    let kept = vm2.apply(
        AutosaveEvent::ResolveConflict(ConflictResolution::KeepMine),
        ms(380),
    );
    let Some(req) = kept.iter().find_map(|eff| match eff {
        AutosaveEffect::Save(req) => Some(req),
        _ => None,
    }) else {
        panic!("expected KeepMine Save, got {kept:?}");
    };
    let _ = vm2.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("local-r2"),
        },
        ms(385),
    );
    assert!(vm2
        .apply(loaded("Test Game", "remote-r1", Some(reload_2)), ms(390))
        .is_empty());
    assert_eq!(vm2.base_revision(), Some(&rev("local-r2")));
}
