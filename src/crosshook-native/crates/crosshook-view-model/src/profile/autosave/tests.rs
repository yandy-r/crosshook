use std::collections::BTreeSet;

use crate::launch::LaunchMethod;
use crate::{ServiceError, ServiceErrorKind};

use super::*;

mod conflict;
mod edge_cases;
mod review_fixes;
mod review_fixes_2;

pub(super) fn ms(n: u64) -> AutosaveInstant {
    AutosaveInstant::from_millis(n)
}

pub(super) fn rev(s: &str) -> RevisionHash {
    RevisionHash::new(s)
}

pub(super) fn saved_scope() -> AutosaveScope {
    AutosaveScope::saved("Test Game", LaunchMethod::ProtonRun)
}

pub(super) fn make_vm(policy: AutosavePolicy) -> ProfileAutosave {
    ProfileAutosave::with_policy(saved_scope(), Some(rev("r0")), policy)
}

#[test]
fn rapid_edits_coalesce_within_debounce_window() {
    let mut vm = make_vm(AutosavePolicy::default());
    let e1 = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    assert_eq!(
        e1,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saving()),
            AutosaveEffect::ScheduleTick(ms(350)),
        ]
    );

    // Edit at 200ms pushes deadline to 550ms, with zero saves emitted yet.
    let e2 = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r2"),
        },
        ms(200),
    );
    assert_eq!(e2, vec![AutosaveEffect::ScheduleTick(ms(550))]);

    // Early tick at 400ms is a no-op.
    assert!(vm.apply(AutosaveEvent::Tick, ms(400)).is_empty());

    // Due tick at 550ms emits exactly one Save for the latest revision.
    let e3 = vm.apply(AutosaveEvent::Tick, ms(550));
    assert_eq!(e3.len(), 1);
    let AutosaveEffect::Save(req) = &e3[0] else {
        panic!("expected Save, got {e3:?}");
    };
    assert_eq!(req.target, SaveTarget::FullProfile);
    assert_eq!(req.revision, rev("r2"));
    assert_eq!(
        req.sections,
        BTreeSet::from([AutosaveSection::ProfileDraft])
    );
}

#[test]
fn environment_uses_400ms_debounce_and_separate_section_grouping() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::Environment,
            revision: rev("env-1"),
        },
        ms(0),
    );
    assert_eq!(vm.scheduled_tick(), Some(ms(400)));
    assert!(vm.apply(AutosaveEvent::Tick, ms(350)).is_empty());
    let e = vm.apply(AutosaveEvent::Tick, ms(400));
    assert!(matches!(
        e.as_slice(),
        [AutosaveEffect::Save(SaveRequest {
            target: SaveTarget::FullProfile,
            ..
        })]
    ));

    // Dedicated launch sections do not trigger FullProfile on their own.
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
    let e1 = vm.apply(AutosaveEvent::Tick, ms(350));
    let AutosaveEffect::Save(req1) = &e1[0] else {
        panic!("expected Gamescope save");
    };
    assert_eq!(req1.target, SaveTarget::Section(SectionSaveKind::Gamescope));
    let after1 = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req1.id,
            committed_rev: rev("gs-1"),
        },
        ms(360),
    );
    assert!(after1.contains(&AutosaveEffect::ScheduleTick(ms(450))));
    let e2 = vm.apply(AutosaveEvent::Tick, ms(450));
    let AutosaveEffect::Save(req2) = &e2[0] else {
        panic!("expected MangoHud save");
    };
    assert_eq!(req2.target, SaveTarget::Section(SectionSaveKind::MangoHud));
}

#[test]
fn max_wait_cap_forces_save_during_continuous_typing() {
    let mut vm = make_vm(AutosavePolicy::default());
    let mut t = 0u64;
    let mut saves = Vec::new();
    for step in 0..11 {
        let out = vm.apply(
            AutosaveEvent::Edit {
                section: AutosaveSection::ProfileDraft,
                revision: rev(&format!("r{}", step + 1)),
            },
            ms(t),
        );
        assert!(
            !out.iter().any(|e| matches!(e, AutosaveEffect::Save(_))),
            "unexpected save on edit at {t}ms"
        );
        t += 200;
    }
    // First edit at 0ms + 2000ms max_wait caps deadline at 2000ms.
    assert_eq!(vm.scheduled_tick(), Some(ms(2_000)));
    saves.extend(vm.apply(AutosaveEvent::Tick, ms(2_000)));
    assert_eq!(saves.len(), 1);
    let AutosaveEffect::Save(req) = &saves[0] else {
        panic!("expected Save at max_wait");
    };
    assert_eq!(req.revision, rev("r11"));
}

#[test]
fn flush_saves_pending_edits_exactly_once() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(10),
    );
    let flushed = vm.apply(AutosaveEvent::Flush, ms(50));
    assert_eq!(flushed.len(), 1);
    let AutosaveEffect::Save(req) = &flushed[0] else {
        panic!("expected immediate Save on Flush");
    };
    assert_eq!(req.revision, rev("r1"));
    assert_eq!(vm.scheduled_tick(), None);

    // A stale timer tick and a second Flush while Idle after SaveSucceeded do nothing.
    assert!(vm.apply(AutosaveEvent::Tick, ms(360)).is_empty());
    let settled = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: req.id,
            committed_rev: rev("r1"),
        },
        ms(80),
    );
    assert!(settled.contains(&AutosaveEffect::ShowStatus(AutosaveStatus::saved())));
    assert!(vm.apply(AutosaveEvent::Flush, ms(90)).is_empty());

    // Dedicated sections flushed together save sequentially without losing the second.
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::LaunchOptimizations,
            revision: rev("opt-1"),
        },
        ms(0),
    );
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::CommandArguments,
            revision: rev("cmd-1"),
        },
        ms(5),
    );
    let f1 = vm.apply(AutosaveEvent::Flush, ms(10));
    let AutosaveEffect::Save(s1) = &f1[0] else {
        panic!("expected first section Save");
    };
    assert_eq!(
        s1.target,
        SaveTarget::Section(SectionSaveKind::LaunchOptimizations)
    );
    let f2 = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: s1.id,
            committed_rev: rev("opt-1"),
        },
        ms(20),
    );
    assert!(matches!(
        f2.as_slice(),
        [AutosaveEffect::Save(SaveRequest {
            target: SaveTarget::Section(SectionSaveKind::CommandArguments),
            ..
        })]
    ));
}

#[test]
fn retry_backoff_ladder_recovers_on_success_and_stops_when_exhausted() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("r1"),
        },
        ms(0),
    );
    let e = vm.apply(AutosaveEvent::Tick, ms(350));
    let AutosaveEffect::Save(mut req) = e[0].clone() else {
        panic!("expected initial save");
    };

    let expected_delays = [1_000u64, 2_000, 4_000, 8_000];
    let mut now = 400u64;
    for (idx, delay) in expected_delays.iter().enumerate() {
        let failed = vm.apply(
            AutosaveEvent::SaveFailed {
                id: req.id,
                error: ServiceError::new(ServiceErrorKind::Io, "disk busy"),
            },
            ms(now),
        );
        let next_at = ms(now + delay);
        assert_eq!(
            failed,
            vec![
                AutosaveEffect::ShowStatus(AutosaveStatus::error(
                    "Failed to save",
                    Some("disk busy".into())
                )),
                AutosaveEffect::ScheduleTick(next_at),
            ]
        );
        assert!(matches!(
            vm.state(),
            AutosaveState::Error {
                attempt,
                retry_at: Some(at),
                ..
            } if *attempt as usize == idx + 1 && *at == next_at
        ));
        now += delay;
        let retry = vm.apply(AutosaveEvent::Tick, ms(now));
        assert_eq!(retry.len(), 2);
        let AutosaveEffect::Save(next_req) = retry[1].clone() else {
            panic!("expected retry Save");
        };
        req = next_req;
        now += 10;
    }

    // 5th failure exhausts the ladder: no ScheduleTick is emitted.
    let exhausted = vm.apply(
        AutosaveEvent::SaveFailed {
            id: req.id,
            error: ServiceError::new(ServiceErrorKind::Io, "still failing"),
        },
        ms(now),
    );
    assert_eq!(
        exhausted,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::error(
            "Failed to save",
            Some("still failing".into())
        ))]
    );
    assert_eq!(vm.scheduled_tick(), None);
    assert!(vm.apply(AutosaveEvent::Tick, ms(now + 10_000)).is_empty());

    // Manual RetryNow + SaveSucceeded clears error state and resets attempts.
    let manual = vm.apply(AutosaveEvent::RetryNow, ms(now + 50));
    let AutosaveEffect::Save(final_req) = manual[1].clone() else {
        panic!("expected manual retry Save");
    };
    let ok = vm.apply(
        AutosaveEvent::SaveSucceeded {
            id: final_req.id,
            committed_rev: rev("r1"),
        },
        ms(now + 80),
    );
    assert_eq!(
        ok,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saved()),
            AutosaveEffect::ScheduleTick(ms(now + 3_080)),
        ]
    );
    assert!(!vm.is_dirty());
}

#[test]
fn non_retryable_and_cancelled_errors_behave_distinctly() {
    let mut vm = make_vm(AutosavePolicy::default());
    let _ = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("bad"),
        },
        ms(0),
    );
    let AutosaveEffect::Save(req) = &vm.apply(AutosaveEvent::Tick, ms(350))[0] else {
        panic!("expected save");
    };
    // Validation errors do not schedule automatic retries.
    let fail = vm.apply(
        AutosaveEvent::SaveFailed {
            id: req.id,
            error: ServiceError::validation("executable_path", "Invalid path"),
        },
        ms(360),
    );
    assert_eq!(
        fail,
        vec![AutosaveEffect::ShowStatus(AutosaveStatus::error(
            "Failed to save",
            Some("Invalid path".into())
        ))]
    );
    assert_eq!(vm.scheduled_tick(), None);

    // Editing to fix validation clears Error and schedules a normal debounce.
    let fix = vm.apply(
        AutosaveEvent::Edit {
            section: AutosaveSection::ProfileDraft,
            revision: rev("good"),
        },
        ms(500),
    );
    assert_eq!(
        fix,
        vec![
            AutosaveEffect::ShowStatus(AutosaveStatus::saving()),
            AutosaveEffect::ScheduleTick(ms(850)),
        ]
    );

    // Cancelled errors do not enter Error state; they return to Dirty.
    let AutosaveEffect::Save(req2) = &vm.apply(AutosaveEvent::Tick, ms(850))[0] else {
        panic!("expected save");
    };
    let cancelled = vm.apply(
        AutosaveEvent::SaveFailed {
            id: req2.id,
            error: ServiceError::new(ServiceErrorKind::Cancelled, "cancelled"),
        },
        ms(860),
    );
    assert!(matches!(vm.state(), AutosaveState::Dirty { .. }));
    assert!(cancelled.contains(&AutosaveEffect::ScheduleTick(ms(1_210))));
}
