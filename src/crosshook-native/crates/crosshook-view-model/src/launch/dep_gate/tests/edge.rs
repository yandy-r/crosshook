//! Gate behaviour beyond the Vitest cases: the acceptance effect sequences
//! (missing, install success, install failure then retry, skip) and edge
//! cases.

use super::*;

#[test]
fn missing_packages_open_the_modal_then_skip_launches() {
    at_package_check(gate(&["vcrun2019", "dotnet48"], false), LaunchAction::Game)
        .step(
            deps_loaded(1, Ok(vec![dep("vcrun2019", DependencyState::Installed)])),
            [],
        )
        .assert_state(|vm| assert_eq!(vm.missing_packages(), Some(&["dotnet48".to_owned()][..])))
        .step(GateIntent::Skip, [GateEffect::Launch(LaunchAction::Game)])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn missing_packages_cancel_clears_the_gate() {
    at_package_check(gate(&["dotnet48"], false), LaunchAction::Trainer)
        .step(deps_loaded(1, Ok(vec![])), [])
        .step(GateIntent::Cancel, [])
        .assert_state(is_idle)
        .step(GateIntent::Skip, [])
        .step(GateIntent::InstallDependencies, [])
        .into_inner();
}

#[test]
fn manual_install_success_launches() {
    at_package_check(gate(&["dotnet48"], false), LaunchAction::Game)
        .step(deps_loaded(1, Ok(vec![])), [])
        .step(GateIntent::InstallDependencies, [install(1, &["dotnet48"])])
        .step(GateIntent::Cancel, [])
        .step(
            completed(true, RestoreState::NotRequired, None),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn manual_install_failure_returns_to_the_modal_and_can_retry() {
    at_package_check(gate(&["dotnet48"], false), LaunchAction::Game)
        .step(deps_loaded(1, Ok(vec![])), [])
        .step(GateIntent::InstallDependencies, [install(1, &["dotnet48"])])
        .step(
            GateIntent::InstallRequestFinished {
                op: OpId(1),
                result: Err(error("spawn failed")),
            },
            [],
        )
        .assert_state(|vm| {
            assert!(matches!(
                vm.state(),
                GateState::Missing {
                    action: LaunchAction::Game,
                    ..
                }
            ));
        })
        .step(GateIntent::InstallDependencies, [install(1, &["dotnet48"])])
        .step(
            completed(true, RestoreState::NotRequired, None),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .into_inner();
}

#[test]
fn failed_install_then_retry_starts_a_fresh_check() {
    auto_installing(LaunchAction::Game)
        .step(completed(false, RestoreState::NotRequired, None), [])
        .assert_state(is_idle)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(2)],
        )
        .into_inner();
}

#[test]
fn auto_install_request_failure_resets_the_gate() {
    at_package_check(gate(&["dotnet48"], true), LaunchAction::Game)
        .step(deps_loaded(1, Ok(vec![])), [install(1, &["dotnet48"])])
        .step(
            GateIntent::InstallRequestFinished {
                op: OpId(1),
                result: Err(error("busy")),
            },
            [],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn no_prefix_launches_without_any_check() {
    let mut vm = DepGate::new(scope_for(NAME, "", &["dotnet48"], false));
    assert_eq!(
        vm.reduce(GateIntent::RequestLaunch(LaunchAction::Trainer)),
        [GateEffect::Launch(LaunchAction::Trainer)]
    );
    is_idle(&vm);
}

#[test]
fn duplicate_required_packages_stay_duplicated_like_classic() {
    at_package_check(gate(&["dotnet48", "dotnet48"], false), LaunchAction::Game)
        .step(
            deps_loaded(1, Ok(vec![dep("dotnet48", DependencyState::Missing)])),
            [],
        )
        .assert_state(|vm| {
            assert_eq!(
                vm.missing_packages(),
                Some(&["dotnet48".to_owned(), "dotnet48".to_owned()][..])
            );
        })
        .into_inner();
}

#[test]
fn check_failed_and_user_skipped_do_not_block_but_unknown_and_install_failed_do() {
    let statuses = vec![
        dep("a", DependencyState::CheckFailed),
        dep("b", DependencyState::UserSkipped),
        dep("c", DependencyState::Unknown),
        dep("d", DependencyState::InstallFailed),
    ];
    at_package_check(gate(&["a", "b", "c", "d", "e"], false), LaunchAction::Game)
        .step(deps_loaded(1, Ok(statuses)), [])
        .assert_state(|vm| {
            let expected: Vec<String> = ["c", "d", "e"].map(String::from).to_vec();
            assert_eq!(vm.missing_packages(), Some(&expected[..]));
        })
        .into_inner();
}

#[test]
fn completion_for_another_profile_or_prefix_is_ignored() {
    let other = |name: &str, prefix: &str| {
        GateIntent::InstallCompleted(InstallCompletion {
            profile_name: name.into(),
            prefix_path: prefix.into(),
            install_succeeded: true,
            restore_state: RestoreState::NotRequired,
            restore_error: None,
        })
    };
    auto_installing(LaunchAction::Game)
        .step(other("Other", PFX), [])
        .step(other(NAME, "/other/pfx"), [])
        .assert_state(|vm| assert!(matches!(vm.state(), GateState::Installing { .. })))
        .step(GateIntent::Cancel, [])
        .assert_state(|vm| assert!(matches!(vm.state(), GateState::Installing { .. })))
        .into_inner();
}

#[test]
fn repair_errors_and_verification_errors_keep_the_repair_modal() {
    let failed = needs_repair(RestoreState::Failed, Some("old"));
    gate(&[], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(failed)), [])
        .step(GateIntent::Repair, [repair(2)])
        .step(GateIntent::Cancel, [])
        .step(
            GateIntent::RepairFinished {
                op: OpId(2),
                result: Err(error("winetricks missing")),
            },
            [],
        )
        .assert_state(|vm| {
            assert_eq!(
                vm.repair_status(),
                Some(&needs_repair(
                    RestoreState::Failed,
                    Some("winetricks missing")
                ))
            );
        })
        .step(GateIntent::Repair, [repair(3)])
        .step(
            GateIntent::RepairFinished {
                op: OpId(3),
                result: Ok(clean()),
            },
            [check_repair(3)],
        )
        .step(status_loaded(3, Err(error("journal locked"))), [])
        .assert_state(|vm| {
            let expected = "Unable to verify prefix repair status: journal locked";
            assert_eq!(
                vm.repair_status().and_then(|s| s.last_error.as_deref()),
                Some(expected)
            );
        })
        .into_inner();
}

#[test]
fn scope_update_for_the_same_target_keeps_the_pending_check() {
    at_package_check(gate(&["dotnet48"], false), LaunchAction::Game)
        .step(
            GateIntent::ScopeChanged(scope_for(NAME, PFX, &["vcrun2019"], false)),
            [],
        )
        .step(deps_loaded(1, Ok(vec![])), [])
        .assert_state(|vm| assert_eq!(vm.missing_packages(), Some(&["vcrun2019".to_owned()][..])))
        .into_inner();
}

fn busy() -> RepairStatus {
    RepairStatus {
        in_progress: true,
        ..needs_repair(RestoreState::Pending, None)
    }
}

/// Request `Game` and land on the repair modal (op 1 done).
fn at_repair_modal(packages: &[&str], auto: bool) -> Script<DepGate> {
    gate(packages, auto)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(1)],
        )
        .step(
            status_loaded(1, Ok(needs_repair(RestoreState::Failed, None))),
            [],
        )
}

fn is_busy_with(action: LaunchAction) -> impl FnOnce(&DepGate) {
    move |vm| {
        assert_eq!(
            vm.state(),
            &GateState::Busy {
                action,
                status: busy()
            }
        );
    }
}

#[test]
fn a_busy_prefix_after_a_repair_shows_busy_not_the_repair_modal() {
    at_repair_modal(&[], false)
        .step(GateIntent::Repair, [repair(2)])
        .step(
            GateIntent::RepairFinished {
                op: OpId(2),
                result: Ok(clean()),
            },
            [check_repair(2)],
        )
        .step(status_loaded(2, Ok(busy())), [])
        .assert_state(is_busy_with(LaunchAction::Game))
        .step(GateIntent::Repair, [])
        .step(GateIntent::Retry, [check_repair(3)])
        // The failed restore seen this session is still unverified, so a clean
        // lookup keeps the repair modal (Classic parity: only a verified repair
        // clears it).
        .step(status_loaded(3, Ok(clean())), [])
        .assert_state(|vm| {
            assert_eq!(
                vm.repair_status(),
                Some(&needs_repair(RestoreState::Failed, None))
            );
        })
        .into_inner();
}

#[test]
fn a_busy_prefix_after_an_install_shows_busy() {
    let mut vm = auto_installing(LaunchAction::Game)
        .step(
            completed(true, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(busy())), [])
        .assert_state(is_busy_with(LaunchAction::Game))
        .into_inner();
    // Busy is not remembered as a repair: a clean re-check launches.
    assert_eq!(vm.reduce(GateIntent::Retry), [check_repair(2)]);
    assert_eq!(
        vm.reduce(deps_loaded(2, Ok(vec![]))),
        [],
        "only the awaited step is accepted"
    );
    assert_eq!(vm.reduce(status_loaded(2, Ok(clean()))), [check_deps(2)]);
}

#[test]
fn a_failed_post_install_verification_blocks_for_repair() {
    auto_installing(LaunchAction::Trainer)
        .step(
            completed(true, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .step(status_loaded(1, Err(error("db locked"))), [])
        .assert_state(|vm| {
            let expected = "Unable to verify prefix repair status: db locked";
            assert_eq!(
                vm.repair_status().and_then(|s| s.last_error.as_deref()),
                Some(expected)
            );
            assert_eq!(vm.pending_action(), Some(LaunchAction::Trainer));
        })
        .into_inner();
}

#[test]
fn a_repair_that_still_needs_repair_keeps_the_modal() {
    let still = needs_repair(RestoreState::Failed, Some("still broken"));
    at_repair_modal(&[], false)
        .step(GateIntent::Repair, [repair(2)])
        .step(
            GateIntent::RepairFinished {
                op: OpId(2),
                result: Ok(still.clone()),
            },
            [],
        )
        .assert_state(|vm| assert_eq!(vm.repair_status(), Some(&still)))
        .step(GateIntent::Repair, [repair(3)])
        .step(
            GateIntent::RepairFinished {
                op: OpId(3),
                result: Ok(clean()),
            },
            [check_repair(3)],
        )
        .step(status_loaded(3, Ok(still.clone())), [])
        .assert_state(|vm| {
            assert!(matches!(
                vm.state(),
                GateState::RepairRequired {
                    repairing: false,
                    ..
                }
            ));
            assert_eq!(vm.repair_status(), Some(&still));
        })
        .into_inner();
}

#[test]
fn a_successful_repair_continues_to_the_package_gate() {
    at_repair_modal(&["dotnet48"], false)
        .step(GateIntent::Repair, [repair(2)])
        .step(
            GateIntent::RepairFinished {
                op: OpId(2),
                result: Ok(clean()),
            },
            [check_repair(2)],
        )
        .step(status_loaded(2, Ok(clean())), [check_deps(2)])
        .step(deps_loaded(2, Ok(vec![])), [])
        .assert_state(|vm| assert_eq!(vm.missing_packages(), Some(&["dotnet48".to_owned()][..])))
        .into_inner();

    at_repair_modal(&["dotnet48"], true)
        .step(GateIntent::Repair, [repair(2)])
        .step(
            GateIntent::RepairFinished {
                op: OpId(2),
                result: Ok(clean()),
            },
            [check_repair(2)],
        )
        .step(status_loaded(2, Ok(clean())), [check_deps(2)])
        .step(deps_loaded(2, Ok(vec![])), [install(2, &["dotnet48"])])
        .into_inner();
}

#[test]
fn retry_keeps_the_busy_modal_ignores_double_retry_and_can_stay_busy() {
    gate(&[], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Trainer),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(busy())), [])
        .step(GateIntent::Retry, [check_repair(2)])
        .assert_state(is_busy_with(LaunchAction::Trainer))
        .step(GateIntent::Retry, [])
        .step(status_loaded(2, Ok(busy())), [])
        .assert_state(is_busy_with(LaunchAction::Trainer))
        .step(GateIntent::Cancel, [])
        .assert_state(is_idle)
        .step(GateIntent::Retry, [])
        .into_inner();
}

#[test]
fn cancel_during_a_check_makes_its_results_stale() {
    gate(&["dotnet48"], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(1)],
        )
        .step(GateIntent::Cancel, [])
        .step(status_loaded(1, Ok(clean())), [])
        .assert_state(is_idle)
        .into_inner();

    at_package_check(gate(&["dotnet48"], false), LaunchAction::Game)
        .step(GateIntent::Cancel, [])
        .step(deps_loaded(1, Err(error("late"))), [])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn launch_requests_are_ignored_while_installing_or_repairing() {
    auto_installing(LaunchAction::Game)
        .step(GateIntent::RequestLaunch(LaunchAction::Trainer), [])
        .step(
            completed(true, RestoreState::NotRequired, None),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .into_inner();

    at_repair_modal(&[], false)
        .step(GateIntent::Repair, [repair(2)])
        .step(GateIntent::RequestLaunch(LaunchAction::Game), [])
        .step(
            GateIntent::RepairFinished {
                op: OpId(2),
                result: Ok(clean()),
            },
            [check_repair(2)],
        )
        .into_inner();
}
