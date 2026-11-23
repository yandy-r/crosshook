//! Conversions from `crosshook-app` / `crosshook-core` types (`app` feature).
//!
//! Every enum `match` is exhaustive on purpose: a new core variant must fail
//! to compile here instead of silently mapping to something else.

use crosshook_app::services::prefix_deps::{DepCompletePayload, PrefixVersionRepairStatus};
use crosshook_core::launch::preview as core_preview;
use crosshook_core::launch::request::{
    LaunchValidationIssue, UmuGameIdResolution, UmuGameIdResolutionSource, ValidationSeverity,
};
use crosshook_core::prefix_deps::{
    DependencyState as CoreDependencyState, PrefixDependencyStatus, PrefixVersionRestoreState,
};
use crosshook_core::profile::GameProfile;
use crosshook_core::umu_database::CsvCoverage as CoreCsvCoverage;

use super::dep_gate::{
    DependencyState, DependencyStatus, GateScope, InstallCompletion, RepairStatus, RestoreState,
};
use super::preview::{
    CsvCoverage, EnvVar, EnvVarSource, GameIdResolution, GameIdSource, LaunchPreview,
    PipelineProfile, PreviewTrainer, PreviewValidation, ProtonSetup, Severity, UmuDecision,
    ValidationIssue, WrapperDetail, WrapperSource,
};
use super::LaunchMethod;

impl PipelineProfile {
    /// Tier-1 pipeline inputs of a profile.
    #[must_use]
    pub fn from_profile(profile: &GameProfile) -> Self {
        Self {
            game_executable_path: profile.game.executable_path.clone(),
            prefix_path: profile.runtime.prefix_path.clone(),
            proton_path: profile.runtime.proton_path.clone(),
            steam_app_id: profile.steam.app_id.clone(),
            trainer_path: profile.trainer.path.clone(),
            enabled_optimizations: profile.launch.optimizations.enabled_option_ids.len(),
        }
    }
}

impl GateScope {
    /// Gate scope of the selected profile `name`.
    #[must_use]
    pub fn from_profile(name: &str, profile: &GameProfile, auto_install: bool) -> Self {
        Self {
            profile_name: name.to_owned(),
            prefix_path: Self::effective_prefix_path(
                &profile.runtime.prefix_path,
                &profile.steam.compatdata_path,
            ),
            required_packages: profile.trainer.required_protontricks.clone(),
            auto_install,
        }
    }
}

impl From<core_preview::ResolvedLaunchMethod> for LaunchMethod {
    fn from(method: core_preview::ResolvedLaunchMethod) -> Self {
        use core_preview::ResolvedLaunchMethod as M;
        match method {
            M::SteamApplaunch => Self::SteamApplaunch,
            M::ProtonRun => Self::ProtonRun,
            M::Native => Self::Native,
        }
    }
}

impl From<ValidationSeverity> for Severity {
    fn from(severity: ValidationSeverity) -> Self {
        match severity {
            ValidationSeverity::Fatal => Self::Fatal,
            ValidationSeverity::Warning => Self::Warning,
            ValidationSeverity::Info => Self::Info,
        }
    }
}

impl From<&LaunchValidationIssue> for ValidationIssue {
    fn from(issue: &LaunchValidationIssue) -> Self {
        Self {
            message: issue.message.clone(),
            severity: issue.severity.into(),
            code: issue.code.clone(),
        }
    }
}

impl From<core_preview::EnvVarSource> for EnvVarSource {
    fn from(source: core_preview::EnvVarSource) -> Self {
        use core_preview::EnvVarSource as S;
        match source {
            S::ProtonRuntime => Self::ProtonRuntime,
            S::LaunchOptimization => Self::LaunchOptimization,
            S::Host => Self::Host,
            S::SteamProton => Self::SteamProton,
            S::ProfileCustom => Self::ProfileCustom,
        }
    }
}

impl From<core_preview::PreviewWrapperSource> for WrapperSource {
    fn from(source: core_preview::PreviewWrapperSource) -> Self {
        use core_preview::PreviewWrapperSource as S;
        match source {
            S::Optimization => Self::Optimization,
            S::NetworkIsolation => Self::NetworkIsolation,
            S::Gamescope => Self::Gamescope,
        }
    }
}

impl From<CoreCsvCoverage> for CsvCoverage {
    fn from(coverage: CoreCsvCoverage) -> Self {
        match coverage {
            CoreCsvCoverage::Found => Self::Found,
            CoreCsvCoverage::Missing => Self::Missing,
            CoreCsvCoverage::Unknown => Self::Unknown,
        }
    }
}

impl From<&UmuGameIdResolutionSource> for GameIdSource {
    fn from(source: &UmuGameIdResolutionSource) -> Self {
        use UmuGameIdResolutionSource as S;
        match source {
            S::ExplicitOverride => Self::ExplicitOverride,
            S::SteamAppId => Self::SteamAppId,
            S::FreshCache => Self::FreshCache,
            S::FreshLookup => Self::FreshLookup,
            S::StaleCache => Self::StaleCache,
            S::CachedNotFound => Self::CachedNotFound,
            S::LookupDisabled => Self::LookupDisabled,
            S::MissingHints => Self::MissingHints,
            S::ApiUnavailable => Self::ApiUnavailable,
            S::Fallback => Self::Fallback,
        }
    }
}

impl From<&UmuGameIdResolution> for GameIdResolution {
    fn from(resolution: &UmuGameIdResolution) -> Self {
        Self {
            game_id: resolution.game_id.clone(),
            source: (&resolution.source).into(),
        }
    }
}

impl From<&core_preview::LaunchPreview> for LaunchPreview {
    fn from(preview: &core_preview::LaunchPreview) -> Self {
        Self {
            resolved_method: preview.resolved_method.into(),
            validation: PreviewValidation {
                issues: preview.validation.issues.iter().map(Into::into).collect(),
            },
            environment: preview.environment.as_ref().map(|vars| {
                vars.iter()
                    .map(|var| EnvVar {
                        key: var.key.clone(),
                        value: var.value.clone(),
                        source: var.source.into(),
                    })
                    .collect()
            }),
            cleared_variables: preview.cleared_variables.clone(),
            wrappers: preview.wrappers.clone(),
            wrapper_details: preview.wrapper_details.as_ref().map(|details| {
                details
                    .iter()
                    .map(|detail| WrapperDetail {
                        command: detail.command.clone(),
                        source: detail.source.into(),
                        active: detail.active,
                        reason: detail.reason.clone(),
                        folded_into: detail.folded_into.clone(),
                    })
                    .collect()
            }),
            effective_command: preview.effective_command.clone(),
            directives_error: preview.directives_error.clone(),
            steam_launch_options: preview.steam_launch_options.clone(),
            proton_setup: preview.proton_setup.as_ref().map(|setup| ProtonSetup {
                wine_prefix_path: setup.wine_prefix_path.clone(),
                proton_executable: setup.proton_executable.clone(),
            }),
            game_executable: preview.game_executable.clone(),
            game_executable_name: preview.game_executable_name.clone(),
            trainer: preview.trainer.as_ref().map(|trainer| PreviewTrainer {
                path: trainer.path.clone(),
            }),
            umu_decision: preview.umu_decision.as_ref().map(|umu| UmuDecision {
                will_use_umu: umu.will_use_umu,
                reason: umu.reason.clone(),
                csv_coverage: umu.csv_coverage.into(),
                gameid_resolution: umu.gameid_resolution.as_ref().map(Into::into),
            }),
        }
    }
}

impl From<PrefixVersionRestoreState> for RestoreState {
    fn from(state: PrefixVersionRestoreState) -> Self {
        match state {
            PrefixVersionRestoreState::NotRequired => Self::NotRequired,
            PrefixVersionRestoreState::Pending => Self::Pending,
            PrefixVersionRestoreState::Succeeded => Self::Succeeded,
            PrefixVersionRestoreState::Failed => Self::Failed,
        }
    }
}

impl From<&PrefixVersionRepairStatus> for RepairStatus {
    fn from(status: &PrefixVersionRepairStatus) -> Self {
        Self {
            required: status.required,
            state: status.state.map(Into::into),
            last_error: status.last_error.clone(),
            in_progress: status.in_progress,
        }
    }
}

impl From<&CoreDependencyState> for DependencyState {
    fn from(state: &CoreDependencyState) -> Self {
        match state {
            CoreDependencyState::Unknown => Self::Unknown,
            CoreDependencyState::Installed => Self::Installed,
            CoreDependencyState::Missing => Self::Missing,
            CoreDependencyState::InstallFailed => Self::InstallFailed,
            CoreDependencyState::CheckFailed => Self::CheckFailed,
            CoreDependencyState::UserSkipped => Self::UserSkipped,
        }
    }
}

impl From<&PrefixDependencyStatus> for DependencyStatus {
    fn from(status: &PrefixDependencyStatus) -> Self {
        Self {
            package_name: status.package_name.clone(),
            state: (&status.state).into(),
        }
    }
}

impl From<&DepCompletePayload> for InstallCompletion {
    fn from(payload: &DepCompletePayload) -> Self {
        Self {
            profile_name: payload.profile_name.clone(),
            prefix_path: payload.prefix_path.clone(),
            install_succeeded: payload.install_succeeded,
            restore_state: payload.restore_state.into(),
            restore_error: payload.restore_error.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
