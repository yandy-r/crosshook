//! Dependency-gate scenarios. Each `vitest_*` test replays one case of
//! `useLaunchDepGate.test.tsx` / `useLaunchDepGate.busy.test.tsx` (named
//! after it) as an intent/effect script; the React hook's subscription and
//! unmount races become stale-`OpId` results here.

use super::*;
use crate::testing::Script;
use crate::ServiceErrorKind;

const NAME: &str = "Synthetic Quest";
const PFX: &str = "/mock/pfx";

/// Operation `seq` of a test gate (mount 0).
fn op(seq: u64) -> OpId {
    OpId { mount: 0, seq }
}

fn scope_for(name: &str, prefix: &str, packages: &[&str], auto_install: bool) -> GateScope {
    GateScope {
        profile_name: name.into(),
        prefix_path: prefix.into(),
        required_packages: packages.iter().map(|p| (*p).to_owned()).collect(),
        auto_install,
    }
}

fn gate(packages: &[&str], auto_install: bool) -> Script<DepGate> {
    Script::new(DepGate::with_mount(
        scope_for(NAME, PFX, packages, auto_install),
        0,
    ))
}

fn check_repair(op: u64) -> GateEffect {
    GateEffect::CheckRepairStatus {
        op: OpId { mount: 0, seq: op },
        profile_name: NAME.into(),
        prefix_path: PFX.into(),
    }
}

fn check_deps(op: u64) -> GateEffect {
    GateEffect::CheckDependencies {
        op: OpId { mount: 0, seq: op },
        profile_name: NAME.into(),
        prefix_path: PFX.into(),
    }
}

fn install(op: u64, packages: &[&str]) -> GateEffect {
    GateEffect::InstallDependencies {
        op: OpId { mount: 0, seq: op },
        profile_name: NAME.into(),
        prefix_path: PFX.into(),
        packages: packages.iter().map(|p| (*p).to_owned()).collect(),
    }
}

fn repair(op: u64) -> GateEffect {
    GateEffect::RepairPrefixVersion {
        op: OpId { mount: 0, seq: op },
        profile_name: NAME.into(),
        prefix_path: PFX.into(),
    }
}

fn error(message: &str) -> ServiceError {
    ServiceError::new(ServiceErrorKind::Unavailable, message)
}

fn clean() -> RepairStatus {
    RepairStatus::default()
}

fn needs_repair(state: RestoreState, last_error: Option<&str>) -> RepairStatus {
    RepairStatus {
        required: true,
        state: Some(state),
        last_error: last_error.map(str::to_owned),
        in_progress: false,
    }
}

fn status_loaded(op: u64, result: Result<RepairStatus, ServiceError>) -> GateIntent {
    GateIntent::RepairStatusLoaded {
        op: OpId { mount: 0, seq: op },
        result,
    }
}

fn deps_loaded(op: u64, result: Result<Vec<DependencyStatus>, ServiceError>) -> GateIntent {
    GateIntent::DependenciesLoaded {
        op: OpId { mount: 0, seq: op },
        result,
    }
}

fn dep(name: &str, state: DependencyState) -> DependencyStatus {
    DependencyStatus {
        package_name: name.into(),
        state,
    }
}

fn completed(
    install_succeeded: bool,
    restore_state: RestoreState,
    restore_error: Option<&str>,
) -> GateIntent {
    GateIntent::InstallCompleted(InstallCompletion {
        profile_name: NAME.into(),
        prefix_path: PFX.into(),
        install_succeeded,
        restore_state,
        restore_error: restore_error.map(str::to_owned),
    })
}

/// Request `action`, pass the repair check, and reach the package check (op 1).
fn at_package_check(script: Script<DepGate>, action: LaunchAction) -> Script<DepGate> {
    script
        .step(GateIntent::RequestLaunch(action), [check_repair(1)])
        .step(status_loaded(1, Ok(clean())), [check_deps(1)])
}

/// Auto-install `dotnet48` for `action` (op 1), install request accepted.
fn auto_installing(action: LaunchAction) -> Script<DepGate> {
    at_package_check(gate(&["dotnet48"], true), action)
        .step(deps_loaded(1, Ok(vec![])), [install(1, &["dotnet48"])])
        .step(
            GateIntent::InstallRequestFinished {
                op: op(1),
                result: Ok(()),
            },
            [],
        )
}

fn is_idle(vm: &DepGate) {
    assert_eq!(vm.state(), &GateState::Idle);
    assert_eq!(vm.pending_action(), None);
}

#[test]
fn vitest_silent_catches_dependency_status_failures_and_allows_launch() {
    at_package_check(gate(&["vcrun2019"], false), LaunchAction::Game)
        .step(
            deps_loaded(1, Err(error("network error"))),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_ignores_prefix_dep_complete_while_the_modal_is_closed() {
    gate(&["vcrun2019"], false)
        .step(completed(true, RestoreState::Succeeded, None), [])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_checks_required_prefix_repair_before_packages_even_when_no_packages_are_declared() {
    let status = needs_repair(RestoreState::Pending, None);
    gate(&[], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(status.clone())), [])
        .assert_state(|vm| {
            assert_eq!(vm.repair_status(), Some(&status));
            assert_eq!(vm.pending_action(), Some(LaunchAction::Game));
        })
        .into_inner();
}

#[test]
fn vitest_resumes_the_remembered_launch_only_after_repair_and_package_recheck_succeed() {
    let failed = needs_repair(RestoreState::Failed, Some("restore timed out"));
    gate(&["vcrun2019"], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(failed)), [])
        .step(GateIntent::Repair, [repair(2)])
        .assert_state(|vm| {
            assert!(matches!(
                vm.state(),
                GateState::RepairRequired {
                    repairing: true,
                    ..
                }
            ));
        })
        .step(
            GateIntent::RepairFinished {
                op: op(2),
                result: Ok(clean()),
            },
            [check_repair(2)],
        )
        .step(status_loaded(2, Ok(clean())), [check_deps(2)])
        .step(
            deps_loaded(2, Ok(vec![dep("vcrun2019", DependencyState::Installed)])),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_keeps_launch_blocked_and_preserves_the_pending_action_when_restoration_fails() {
    auto_installing(LaunchAction::Trainer)
        .step(
            completed(true, RestoreState::Failed, Some("restore timed out")),
            [],
        )
        .assert_state(|vm| {
            assert_eq!(
                vm.repair_status(),
                Some(&needs_repair(
                    RestoreState::Failed,
                    Some("restore timed out")
                ))
            );
            assert_eq!(vm.pending_action(), Some(LaunchAction::Trainer));
            assert_eq!(vm.missing_packages(), None);
        })
        .into_inner();
}

#[test]
fn vitest_does_not_launch_when_installation_fails_after_compatibility_restoration_succeeds() {
    auto_installing(LaunchAction::Game)
        .step(
            completed(false, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(clean())), [])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_resumes_the_remembered_action_when_completion_arrives_during_the_install_ipc_call() {
    at_package_check(gate(&["dotnet48"], true), LaunchAction::Trainer)
        .step(deps_loaded(1, Ok(vec![])), [install(1, &["dotnet48"])])
        .step(
            completed(true, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(clean())), [check_deps(1)])
        .step(
            deps_loaded(1, Ok(vec![dep("dotnet48", DependencyState::Installed)])),
            [GateEffect::Launch(LaunchAction::Trainer)],
        )
        .step(
            GateIntent::InstallRequestFinished {
                op: op(1),
                result: Ok(()),
            },
            [],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_uses_the_steam_compatdata_prefix_when_the_runtime_prefix_is_blank() {
    let prefix = GateScope::effective_prefix_path("   ", "  /steam/compatdata/123/pfx  ");
    assert_eq!(prefix, "/steam/compatdata/123/pfx");
    assert_eq!(GateScope::effective_prefix_path(" /a ", "/b"), "/a");
    assert_eq!(GateScope::effective_prefix_path(" ", " "), "");

    let mut vm = DepGate::with_mount(scope_for(NAME, &prefix, &[], false), 0);
    assert_eq!(
        vm.reduce(GateIntent::RequestLaunch(LaunchAction::Game)),
        [GateEffect::CheckRepairStatus {
            op: op(1),
            profile_name: NAME.into(),
            prefix_path: "/steam/compatdata/123/pfx".into(),
        }]
    );
}

#[test]
fn vitest_keeps_a_same_session_failed_repair_non_skippable_after_cancel_and_lookup_failure() {
    let failed = needs_repair(RestoreState::Failed, Some("restore timed out"));
    gate(&["dotnet48"], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Trainer),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(failed.clone())), [])
        .step(GateIntent::Cancel, [])
        .assert_state(is_idle)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Trainer),
            [check_repair(2)],
        )
        .step(
            status_loaded(2, Err(error("repair journal unavailable"))),
            [],
        )
        .assert_state(|vm| {
            assert_eq!(vm.repair_status(), Some(&failed));
            assert_eq!(vm.pending_action(), Some(LaunchAction::Trainer));
        })
        .into_inner();
}

#[test]
fn vitest_rechecks_the_journal_before_resuming_after_a_successful_restore_event() {
    let journal = needs_repair(RestoreState::Failed, Some("journal cleanup failed"));
    auto_installing(LaunchAction::Game)
        .step(
            completed(true, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(journal.clone())), [])
        .assert_state(|vm| {
            assert_eq!(vm.repair_status(), Some(&journal));
            assert_eq!(vm.pending_action(), Some(LaunchAction::Game));
        })
        .into_inner();
}

#[test]
fn vitest_keeps_the_dependency_gate_busy_until_post_restore_verification_completes() {
    auto_installing(LaunchAction::Game)
        .step(
            completed(true, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .assert_state(|vm| {
            assert!(matches!(
                vm.state(),
                GateState::Installing {
                    verifying: true,
                    ..
                }
            ));
            assert_eq!(vm.missing_packages(), Some(&["dotnet48".to_owned()][..]));
        })
        .step(status_loaded(1, Ok(clean())), [check_deps(1)])
        .step(
            deps_loaded(1, Ok(vec![dep("dotnet48", DependencyState::Installed)])),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_does_not_launch_or_mutate_gate_state_when_profile_changes_during_completion_verification()
{
    auto_installing(LaunchAction::Trainer)
        .step(
            completed(true, RestoreState::Succeeded, None),
            [check_repair(1)],
        )
        .step(
            GateIntent::ScopeChanged(scope_for("Profile B", "/prefix/b", &[], true)),
            [],
        )
        .step(status_loaded(1, Ok(clean())), [])
        .assert_state(|vm| {
            is_idle(vm);
            assert_eq!(vm.missing_packages(), None);
            assert_eq!(vm.repair_status(), None);
        })
        .into_inner();
}

/// Unmount: the bridge drops the gate together with its pending tasks, so a
/// result can only reach a remounted gate, which never asked for it.
fn remounted() -> Script<DepGate> {
    gate(&["dotnet48"], true)
}

#[test]
fn vitest_does_not_launch_after_unmount_during_completion_verification() {
    // The unmounted gate was verifying after a successful restore (op 1).
    remounted()
        .step(status_loaded(1, Ok(clean())), [])
        .step(completed(true, RestoreState::Succeeded, None), [])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_unsubscribes_and_skips_install_when_unmounted_before_subscription_resolves() {
    remounted()
        .step(deps_loaded(1, Ok(vec![])), [])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_does_not_authorize_launch_when_dependency_status_rejects_after_unmount() {
    remounted()
        .step(
            deps_loaded(1, Err(error("dependency cache unavailable"))),
            [],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_unsubscribes_and_skips_install_when_profile_changes_before_subscription_resolves() {
    at_package_check(gate(&["dotnet48"], true), LaunchAction::Game)
        .step(
            GateIntent::ScopeChanged(scope_for("Profile B", "/prefix/b", &[], true)),
            [],
        )
        .step(deps_loaded(1, Ok(vec![])), [])
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_does_not_authorize_launch_when_dependency_status_rejects_after_a_profile_change() {
    at_package_check(gate(&["dotnet48"], false), LaunchAction::Game)
        .step(
            GateIntent::ScopeChanged(scope_for("Profile B", "/prefix/b", &[], false)),
            [],
        )
        .step(
            deps_loaded(1, Err(error("dependency cache unavailable"))),
            [],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_ignores_an_older_install_rejection_without_clearing_the_newer_profile_gate() {
    let b = |op| GateEffect::CheckRepairStatus {
        op: OpId { mount: 0, seq: op },
        profile_name: "Profile B".into(),
        prefix_path: "/prefix/b".into(),
    };
    auto_installing(LaunchAction::Game)
        .step(
            GateIntent::ScopeChanged(scope_for("Profile B", "/prefix/b", &["dotnet48"], true)),
            [],
        )
        .step(GateIntent::RequestLaunch(LaunchAction::Trainer), [b(2)])
        .step(
            status_loaded(2, Ok(clean())),
            [GateEffect::CheckDependencies {
                op: op(2),
                profile_name: "Profile B".into(),
                prefix_path: "/prefix/b".into(),
            }],
        )
        .step(
            deps_loaded(2, Ok(vec![])),
            [GateEffect::InstallDependencies {
                op: op(2),
                profile_name: "Profile B".into(),
                prefix_path: "/prefix/b".into(),
                packages: vec!["dotnet48".into()],
            }],
        )
        .step(
            GateIntent::InstallRequestFinished {
                op: op(1),
                result: Err(error("old install failed")),
            },
            [],
        )
        .assert_state(|vm| {
            assert!(matches!(
                vm.state(),
                GateState::Installing {
                    action: LaunchAction::Trainer,
                    auto: true,
                    verifying: false,
                    ..
                }
            ));
            assert_eq!(vm.missing_packages(), Some(&["dotnet48".to_owned()][..]));
        })
        .step(
            GateIntent::InstallCompleted(InstallCompletion {
                profile_name: "Profile B".into(),
                prefix_path: "/prefix/b".into(),
                install_succeeded: true,
                restore_state: RestoreState::NotRequired,
                restore_error: None,
            }),
            [GateEffect::CheckDependencies {
                op: op(2),
                profile_name: "Profile B".into(),
                prefix_path: "/prefix/b".into(),
            }],
        )
        .step(
            deps_loaded(2, Ok(vec![dep("dotnet48", DependencyState::Installed)])),
            [GateEffect::Launch(LaunchAction::Trainer)],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_blocks_for_repair_when_the_restore_outcome_is_pending_even_if_the_status_lookup_later_fails(
) {
    let pending = needs_repair(RestoreState::Pending, None);
    auto_installing(LaunchAction::Game)
        .step(completed(false, RestoreState::Pending, None), [])
        .assert_state(|vm| {
            assert_eq!(vm.repair_status(), Some(&pending));
            assert_eq!(vm.pending_action(), Some(LaunchAction::Game));
        })
        .step(GateIntent::Cancel, [])
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(2)],
        )
        .step(
            status_loaded(2, Err(error("repair journal unavailable"))),
            [],
        )
        .assert_state(|vm| assert_eq!(vm.repair_status(), Some(&pending)))
        .into_inner();
}

#[test]
fn vitest_shows_a_busy_state_instead_of_the_repair_flow_while_a_job_holds_the_prefix_then_retries()
{
    let busy = RepairStatus {
        in_progress: true,
        ..needs_repair(RestoreState::Pending, None)
    };
    gate(&[], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Trainer),
            [check_repair(1)],
        )
        .step(status_loaded(1, Ok(busy.clone())), [])
        .assert_state(|vm| {
            assert_eq!(
                vm.state(),
                &GateState::Busy {
                    action: LaunchAction::Trainer,
                    status: busy.clone()
                }
            );
        })
        .step(GateIntent::Repair, [])
        .step(GateIntent::Retry, [check_repair(2)])
        .step(
            status_loaded(2, Ok(clean())),
            [GateEffect::Launch(LaunchAction::Trainer)],
        )
        .assert_state(is_idle)
        .into_inner();
}

#[test]
fn vitest_launches_normally_while_a_job_holds_the_prefix_without_a_repair_row() {
    let held = RepairStatus {
        in_progress: true,
        ..clean()
    };
    gate(&[], false)
        .step(
            GateIntent::RequestLaunch(LaunchAction::Game),
            [check_repair(1)],
        )
        .step(
            status_loaded(1, Ok(held)),
            [GateEffect::Launch(LaunchAction::Game)],
        )
        .assert_state(is_idle)
        .into_inner();
}

mod edge;
