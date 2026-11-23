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
