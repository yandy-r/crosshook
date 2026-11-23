//! Prefix dependency gate: decides whether a game or trainer launch may start
//! now, must wait for a prefix repair or a busy prefix, or needs missing
//! prefix dependencies installed first (`useLaunchDepGate.ts`).
//!
//! The gate is a pure state machine. Every service call (repair status,
//! dependency status, install, repair) is a [`GateEffect`] carrying an
//! [`OpId`]; the bridge runs it through `crosshook-app` and returns the result
//! as a [`GateIntent`] with the same id. Starting a new check, changing the
//! scope or cancelling makes older ids stale, and stale results are ignored,
//! so a late reply can never launch or change the gate. [`GateEffect::Launch`]
//! means "the launch may start now".
//!
//! | State            | Shown as                              | Leaves on                               |
//! | ---------------- | ------------------------------------- | --------------------------------------- |
//! | `Idle`           | nothing                               | `RequestLaunch`                         |
//! | `Checking`       | nothing (launch pending)              | status results                          |
//! | `Missing`        | "Missing prefix dependencies" modal   | `InstallDependencies`, `Skip`, `Cancel` |
//! | `Installing`     | same modal, busy (then "verifying")   | `InstallCompleted`, install failure     |
//! | `RepairRequired` | "Prefix repair required" modal        | `Repair`, `Cancel`                      |
//! | `Busy`           | "Prefix busy" modal                   | `Retry`, `Cancel`                       |
//!
//! A repair requirement seen in this session (a failed or pending restore) is
//! remembered per prefix, so the launch stays blocked when a later status
//! lookup fails; any other status-lookup failure is fail-soft (the check is
//! skipped, Classic parity).

use std::collections::BTreeMap;

use super::LaunchAction;
use crate::{ServiceError, ViewModel};

mod types;

pub use types::{
    DependencyState, DependencyStatus, GateEffect, GateIntent, GateScope, GateState,
    InstallCompletion, OpId, RepairStatus, RestoreState,
};

/// Result the active operation waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaiting {
    /// Repair status before a launch (`evaluateLaunch`).
    LaunchRepairStatus,
    /// Package status (`evaluatePackageGate`).
    Dependencies,
    /// Repair status after a successful restore (install completion).
    InstallVerify { install_succeeded: bool },
    /// The repair call.
    Repair,
    /// Repair status after a repair.
    RepairVerify,
}

/// The prefix dependency gate of one launch panel.
#[derive(Debug, Clone, Default)]
pub struct DepGate {
    scope: GateScope,
    state: GateState,
    op: Option<OpId>,
    awaiting: Option<Awaiting>,
    last_op: u64,
    /// Repair requirements seen this session, by prefix path.
    known_repairs: BTreeMap<String, RepairStatus>,
}

impl DepGate {
    /// A gate for `scope`, idle.
    #[must_use]
    pub fn new(scope: GateScope) -> Self {
        Self {
            scope,
            ..Self::default()
        }
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> &GateState {
        &self.state
    }

    /// Current scope.
    #[must_use]
    pub fn scope(&self) -> &GateScope {
        &self.scope
    }

    /// Launch waiting on the gate, if any.
    #[must_use]
    pub fn pending_action(&self) -> Option<LaunchAction> {
        match &self.state {
            GateState::Idle => None,
            GateState::Checking { action }
            | GateState::Missing { action, .. }
            | GateState::Installing { action, .. }
            | GateState::RepairRequired { action, .. }
            | GateState::Busy { action, .. } => Some(*action),
        }
    }

    /// Packages the missing-packages modal lists.
    #[must_use]
    pub fn missing_packages(&self) -> Option<&[String]> {
        match &self.state {
            GateState::Missing { packages, .. } | GateState::Installing { packages, .. } => {
                Some(packages)
            }
            _ => None,
        }
    }

    /// Repair status the repair or busy modal shows.
    #[must_use]
    pub fn repair_status(&self) -> Option<&RepairStatus> {
        match &self.state {
            GateState::RepairRequired { status, .. } | GateState::Busy { status, .. } => {
                Some(status)
            }
            _ => None,
        }
    }

    fn begin(&mut self, awaiting: Awaiting) -> OpId {
        self.last_op += 1;
        let op = OpId(self.last_op);
        self.op = Some(op);
        self.awaiting = Some(awaiting);
        op
    }

    fn reset(&mut self) {
        self.state = GateState::Idle;
        self.op = None;
        self.awaiting = None;
    }

    /// Takes the result of `step` for `op` if it is the active one.
    fn accept(&mut self, op: OpId, step: impl Fn(Awaiting) -> bool) -> Option<Awaiting> {
        let awaiting = self.awaiting.filter(|a| self.op == Some(op) && step(*a))?;
        self.awaiting = None;
        Some(awaiting)
    }

    fn effect_target(&self) -> (String, String) {
        (
            self.scope.profile_name.clone(),
            self.scope.prefix_path.clone(),
        )
    }

    fn check_repair(&self, op: OpId) -> GateEffect {
        let (profile_name, prefix_path) = self.effect_target();
        GateEffect::CheckRepairStatus {
            op,
            profile_name,
            prefix_path,
        }
    }

    fn allow(&mut self, action: LaunchAction) -> Vec<GateEffect> {
        self.reset();
        vec![GateEffect::Launch(action)]
    }

    fn block_for_repair(&mut self, action: LaunchAction, status: RepairStatus) -> Vec<GateEffect> {
        self.known_repairs
            .insert(self.scope.prefix_path.clone(), status.clone());
        self.op = None;
        self.state = GateState::RepairRequired {
            action,
            status,
            repairing: false,
        };
        Vec::new()
    }

    fn start_check(&mut self, action: LaunchAction) -> Vec<GateEffect> {
        if self.scope.prefix_path.is_empty() {
            return self.allow(action);
        }
        let op = self.begin(Awaiting::LaunchRepairStatus);
        self.state = GateState::Checking { action };
        vec![self.check_repair(op)]
    }

    /// Package step of the active operation (`evaluatePackageGate`).
    fn check_packages(&mut self, action: LaunchAction) -> Vec<GateEffect> {
        if self.scope.required_packages.is_empty() {
            return self.allow(action);
        }
        let Some(op) = self.op else {
            return Vec::new();
        };
        self.awaiting = Some(Awaiting::Dependencies);
        self.state = GateState::Checking { action };
        let (profile_name, prefix_path) = self.effect_target();
        vec![GateEffect::CheckDependencies {
            op,
            profile_name,
            prefix_path,
        }]
    }

    fn install(
        &mut self,
        action: LaunchAction,
        packages: Vec<String>,
        auto: bool,
    ) -> Vec<GateEffect> {
        let Some(op) = self.op else {
            return Vec::new();
        };
        let (profile_name, prefix_path) = self.effect_target();
        self.state = GateState::Installing {
            action,
            packages: packages.clone(),
            auto,
            verifying: false,
        };
        vec![GateEffect::InstallDependencies {
            op,
            profile_name,
            prefix_path,
            packages,
        }]
    }

    fn on_repair_status(
        &mut self,
        op: OpId,
        result: Result<RepairStatus, ServiceError>,
    ) -> Vec<GateEffect> {
        let Some(action) = self.pending_action() else {
            return Vec::new();
        };
        let is_status_step = |a| {
            matches!(
                a,
                Awaiting::LaunchRepairStatus
                    | Awaiting::InstallVerify { .. }
                    | Awaiting::RepairVerify
            )
        };
        let Some(step) = self.accept(op, is_status_step) else {
            return Vec::new();
        };
        let prefix = self.scope.prefix_path.clone();
        match step {
            Awaiting::LaunchRepairStatus => match result {
                Ok(status) if status.is_busy() => {
                    self.op = None;
                    self.state = GateState::Busy { action, status };
                    Vec::new()
                }
                Ok(status) if status.required => self.block_for_repair(action, status),
                Ok(_) | Err(_) => match self.known_repairs.get(&prefix).cloned() {
                    Some(known) => self.block_for_repair(action, known),
                    None => self.check_packages(action),
                },
            },
            Awaiting::InstallVerify { install_succeeded } => match result {
                Ok(status) if status.is_busy() => {
                    self.op = None;
                    self.state = GateState::Busy { action, status };
                    Vec::new()
                }
                Ok(status) if status.required => self.block_for_repair(action, status),
                Ok(_) => {
                    self.known_repairs.remove(&prefix);
                    if install_succeeded {
                        self.allow(action)
                    } else {
                        self.reset();
                        Vec::new()
                    }
                }
                Err(error) => self.block_for_repair(action, verification_failure(&error)),
            },
            Awaiting::RepairVerify => match result {
                Ok(status) if status.required => self.block_for_repair(action, status),
                Ok(_) => {
                    self.known_repairs.remove(&prefix);
                    self.check_packages(action)
                }
                Err(error) => self.block_for_repair(action, verification_failure(&error)),
            },
            Awaiting::Dependencies | Awaiting::Repair => Vec::new(),
        }
    }

    fn on_dependencies(
        &mut self,
        op: OpId,
        result: Result<Vec<DependencyStatus>, ServiceError>,
    ) -> Vec<GateEffect> {
        let Some(action) = self.pending_action() else {
            return Vec::new();
        };
        if self.accept(op, |a| a == Awaiting::Dependencies).is_none() {
            return Vec::new();
        }
        // A failed lookup cannot tell what is missing: allow the launch.
        let Ok(statuses) = result else {
            return self.allow(action);
        };
        let missing: Vec<String> = self
            .scope
            .required_packages
            .iter()
            .filter(|package| {
                statuses
                    .iter()
                    .find(|s| &s.package_name == *package)
                    .is_none_or(|s| s.state.blocks_launch())
            })
            .cloned()
            .collect();
        if missing.is_empty() {
            self.allow(action)
        } else if self.scope.auto_install {
            self.install(action, missing, true)
        } else {
            self.state = GateState::Missing {
                action,
                packages: missing,
            };
            Vec::new()
        }
    }

    fn on_install_request_finished(
        &mut self,
        op: OpId,
        result: Result<(), ServiceError>,
    ) -> Vec<GateEffect> {
        if result.is_ok() || self.op != Some(op) {
            return Vec::new();
        }
        if let GateState::Installing {
            action,
            packages,
            auto,
            verifying: false,
        } = &self.state
        {
            if *auto {
                self.reset();
            } else {
                self.state = GateState::Missing {
                    action: *action,
                    packages: packages.clone(),
                };
            }
        }
        Vec::new()
    }

    fn on_install_completed(&mut self, done: InstallCompletion) -> Vec<GateEffect> {
        let GateState::Installing {
            action,
            verifying: false,
            ..
        } = self.state
        else {
            return Vec::new();
        };
        if self.op.is_none()
            || done.profile_name != self.scope.profile_name
            || done.prefix_path != self.scope.prefix_path
        {
            return Vec::new();
        }
        match done.restore_state {
            // `Pending`: the restore never completed, so the prefix needs a
            // repair just like a failed one.
            RestoreState::Failed | RestoreState::Pending => self.block_for_repair(
                action,
                RepairStatus {
                    required: true,
                    state: Some(done.restore_state),
                    last_error: done.restore_error,
                    in_progress: false,
                },
            ),
            RestoreState::Succeeded => {
                let Some(op) = self.op else {
                    return Vec::new();
                };
                self.awaiting = Some(Awaiting::InstallVerify {
                    install_succeeded: done.install_succeeded,
                });
                if let GateState::Installing { verifying, .. } = &mut self.state {
                    *verifying = true;
                }
                vec![self.check_repair(op)]
            }
            RestoreState::NotRequired if done.install_succeeded => self.allow(action),
            RestoreState::NotRequired => {
                self.reset();
                Vec::new()
            }
        }
    }

    fn on_repair(&mut self) -> Vec<GateEffect> {
        let GateState::RepairRequired {
            repairing: false, ..
        } = self.state
        else {
            return Vec::new();
        };
        let op = self.begin(Awaiting::Repair);
        if let GateState::RepairRequired { repairing, .. } = &mut self.state {
            *repairing = true;
        }
        let (profile_name, prefix_path) = self.effect_target();
        vec![GateEffect::RepairPrefixVersion {
            op,
            profile_name,
            prefix_path,
        }]
    }

    fn on_repair_finished(
        &mut self,
        op: OpId,
        result: Result<RepairStatus, ServiceError>,
    ) -> Vec<GateEffect> {
        let Some(action) = self.pending_action() else {
            return Vec::new();
        };
        if self.accept(op, |a| a == Awaiting::Repair).is_none() {
            return Vec::new();
        }
        match result {
            Ok(status) if status.required => self.block_for_repair(action, status),
            Ok(_) => {
                self.awaiting = Some(Awaiting::RepairVerify);
                vec![self.check_repair(op)]
            }
            Err(error) => self.block_for_repair(action, RepairStatus::failed(error.message)),
        }
    }
}

fn verification_failure(error: &ServiceError) -> RepairStatus {
    RepairStatus::failed(format!(
        "Unable to verify prefix repair status: {}",
        error.message
    ))
}

impl ViewModel for DepGate {
    type Intent = GateIntent;
    type Effect = GateEffect;

    fn reduce(&mut self, intent: GateIntent) -> Vec<GateEffect> {
        match intent {
            GateIntent::ScopeChanged(scope) => {
                if !self.scope.same_target(&scope) {
                    self.reset();
                }
                self.scope = scope;
                Vec::new()
            }
            GateIntent::RequestLaunch(action) => self.start_check(action),
            GateIntent::Retry => match self.state {
                GateState::Busy { action, .. } => self.start_check(action),
                _ => Vec::new(),
            },
            GateIntent::RepairStatusLoaded { op, result } => self.on_repair_status(op, result),
            GateIntent::DependenciesLoaded { op, result } => self.on_dependencies(op, result),
            GateIntent::InstallDependencies => match &self.state {
                GateState::Missing { action, packages } => {
                    let (action, packages) = (*action, packages.clone());
                    self.install(action, packages, false)
                }
                _ => Vec::new(),
            },
            GateIntent::InstallRequestFinished { op, result } => {
                self.on_install_request_finished(op, result)
            }
            GateIntent::InstallCompleted(done) => self.on_install_completed(done),
            GateIntent::Skip => match self.state {
                GateState::Missing { action, .. } => self.allow(action),
                _ => Vec::new(),
            },
            GateIntent::Cancel => {
                // Busy controls are disabled while installing or repairing.
                if !matches!(
                    self.state,
                    GateState::Installing { .. }
                        | GateState::RepairRequired {
                            repairing: true,
                            ..
                        }
                ) {
                    self.reset();
                }
                Vec::new()
            }
            GateIntent::Repair => self.on_repair(),
            GateIntent::RepairFinished { op, result } => self.on_repair_finished(op, result),
        }
    }
}

#[cfg(test)]
mod tests;
