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
